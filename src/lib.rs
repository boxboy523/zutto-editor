use std::{collections::HashMap, hash::Hash, io, path::PathBuf, sync::Arc};

use anyhow::{Result, Error};
use key::{open_keymaps, Keymap};
use crossterm::{event::{self, EventStream}, terminal};
use log::debug;
use regex::Regex;
use render::Renderer;
use strum_macros::IntoStaticStr;
use syntect::highlighting::ThemeSet;
use tab::{buffer::Buffer, directory, Pos, Size, Tab};
use tokio::sync::{mpsc::{self, Receiver}, Mutex};
use tokio_stream::StreamExt;
use serde::{de, Deserialize, Serialize};

pub mod key;
pub mod render;
pub mod actions;
pub mod tab;
pub mod lineinput;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum TabType {
    Space,
    Tab,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Setting {
    line_numbers: bool,
    tab_size: usize,
    tab_type: TabType,
    show_spaces: bool,
    theme: String,
}

#[derive(Debug, IntoStaticStr, Clone, Copy, Hash, Serialize, Deserialize,PartialEq, Eq)]
pub enum KeymapState {
    Normal,
    Cmd,
    Find,
    LineInsert,
} 

#[derive(Debug)]
pub struct EventHandler {
    action_channel_tx: tokio::sync::mpsc::Sender<String>,
    keymaps: HashMap<KeymapState, Keymap>,
    reader: EventStream,
    editor: EditorInfo,
}

impl EventHandler {
    pub fn new(action_channel_tx: tokio::sync::mpsc::Sender<String>, editor: EditorInfo) -> Self 
    {
        Self {
            action_channel_tx,
            keymaps: open_keymaps("settings/keymap.json").unwrap(),
            reader: EventStream::new(),
            editor,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        while let Some(event) = self.reader.next().await {
            {
                let running = self.editor.running.lock().await;
                if *running == false {
                    return Ok(());
                }
            }
            let state = self.editor.state.lock().await;
            let keymap = self.keymaps.get(&state).unwrap();
            if let Ok(event) = event {
                match event {
                    event::Event::Key(event) => {
                        let key = Keymap::read(event);
                        if let Some(key) = key {
                            if let Some(action) = keymap.get_action(&key) {
                                self.action_channel_tx.send(action).await?;
                            }
                        }
                    }
                    event::Event::Resize(_, _) => {
                        let size = terminal::size().unwrap();
                        self.editor.size = Size {
                            width: size.0,
                            height: size.1,
                        };
                        self.action_channel_tx.send(
                            format!("Resize({},{})", size.0, size.1)
                        ).await?;
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct EditorInfo
{
    pub size: Size,
    pub setting: Setting,
    pub state: Arc<Mutex<KeymapState>>,
    pub running: Arc<Mutex<bool>>,
    pub alart_tx: mpsc::Sender<Error>,
    pub tabs: Arc<Mutex<Vec<Tab>>>,
    let 
    pub line_input: Arc<Mutex<lineinput::LineInput>>,
}

async fn process_action(
    mut action_rx: Receiver<String>, 
    editor: EditorInfo,
) 
{
    type F = Box<dyn FnMut(&Action) -> Result<Vec<actions::ActionReturn>>>;
    let mut continued = false;
    let mut pre_selected_action = None;
    let mut tab_idx = 0;
    let mut clear = false;
    let mut action_map: HashMap<&str, F>
        = HashMap::new();
    action_map.insert("NormalMode", Box::new(actions::normal_mode));
    action_map.insert("CmdMode", Box::new(actions::cmd_mode));
    action_map.insert("Quit", Box::new(actions::quit));
    action_map.insert("FindMode", Box::new(actions::find_mode));
    action_map.insert("LineMode", Box::new(actions::line_mode));
    action_map.insert("NextTab", Box::new(actions::next_tab));
    action_map.insert("PrevTab", Box::new(actions::prev_tab));
    action_map.insert("Open", Box::new(actions::open));
    action_map.insert("CloseTab", Box::new(actions::close_tab));
    action_map.insert("Shell", Box::new(actions::new_shell));
    
    loop {
        let mut line_input = editor.line_input.lock().await;
        if continued {
            continued = false;
        }
        if clear {
            clear = false;
        }
        let action = if let Some(a) = pre_selected_action {
            pre_selected_action = None;
            a
        } else {
            let action = action_rx.recv().await.unwrap();
            parse_action(&action, &line_input.text, tab_idx).unwrap()
        };
        let mut state = editor.state.lock().await;
        let mut running = editor.running.lock().await;
        let mut tabs = editor.tabs.lock().await;
        let func = action_map.get_mut(action.name.as_str());
        let mut return_queue = Vec::new();
        if let Some(f) = func {
            let returns = match f(&action) {
                Ok(r) => r,
                Err(e) => vec![actions::ActionReturn::Err(e)]
            };
            return_queue.extend(returns);
        };
        return_queue.extend(match tabs[tab_idx] {
            Tab::Buffer(ref mut buffer) => {
                buffer.process_action(&action).await.unwrap()
            }
            Tab::Directory(ref mut directory) => {
                directory.process_action(&action).await.unwrap()
            }
            Tab::Shell(ref mut shell) => {
                shell.process_action(&action).await.unwrap()
        }
        });
        return_queue.extend(renderer.line_input.process_action(&action, tab_idx).unwrap());
        for r in return_queue {
            match r {
                actions::ActionReturn::Stop => {
                    *running = false;
                    return ();
                }
                actions::ActionReturn::Continue => {
                    continued = true;
                }
                actions::ActionReturn::Excute(a) => {
                    pre_selected_action = Some(a);
                }
                actions::ActionReturn::Err(e) => {
                    editor.alart_tx.send(e).await.unwrap();
                }
                actions::ActionReturn::NewBuffer(path) => {
                    let mut size = editor.size;
                    size.height -= 2;
                    match path {
                        Some(path) => {
                            let new_buffer = match Buffer::from_file(size, Pos{row: 1, col: 0}, &path, editor.setting.clone(), tabs.len()) {
                                Ok(b) => b,
                                Err(e) => {
                                    editor.alart_tx.send(e).await.unwrap();
                                    continue;
                                }
                            };
                            tabs.push(Tab::Buffer(new_buffer));
                        }
                        None => {
                            let new_buffer = Buffer::new(size, Pos{row: 1, col: 0}, editor.setting.clone(), tabs.len());
                            tabs.push(Tab::Buffer(new_buffer));
                        }
                    }
                    
                    tab_idx = tabs.len() - 1;
                }
                actions::ActionReturn::State(s) => {
                    *state = s;
                }
                actions::ActionReturn::Notice(s) => {
                    renderer.line_input.notice = s;
                }
                actions::ActionReturn::ExcuteLine(s) => {
                    renderer.line_input.action = Some(s);
                }
                actions::ActionReturn::ChangeTab(i) => {
                    let len = tabs.len() as isize;
                    tab_idx = ((tab_idx as isize + i + len) % len) as usize;
                    clear = true;
                }
                actions::ActionReturn::NewDir(path) => {
                    let mut size = editor.size;
                    size.height -= 2;
                    let new_dir = match directory::Directory::new(path, Pos{row: 1, col: 0}, size, tabs.len()) {
                        Ok(d) => d,
                        Err(e) => {
                            editor.alart_tx.send(e).await.unwrap();
                            continue;
                        }
                    };
                    tabs.push(Tab::Directory(new_dir));
                    tab_idx = tabs.len() - 1;
                }
                actions::ActionReturn::CloseTab(i) => {
                    tabs.remove(i);
                    if tab_idx >= i {
                        tab_idx -= 1;
                    }
                    if tab_idx == i {
                        clear = true;
                    }
                    for i in 0..tabs.len() {
                        match &mut tabs[i] {
                            Tab::Buffer(b) => {
                                b.tab_idx = i;
                            }
                            Tab::Directory(d) => {
                                d.tab_idx = i;
                            }
                            Tab::Shell(s) => {
                                s.tab_idx = i;
                            }
                        }
                    }
                    if tabs.len() == 0 {
                        *running = false;
                        return ();
                    }
                }
                actions::ActionReturn::NewShell => {
                    let mut size = editor.size;
                    size.height -= 2;
                    let shell = tab::shell::Shell::new(Pos{row: 1, col: 0}, size, tabs.len());
                    tabs.push(Tab::Shell(shell));
                    tab_idx = tabs.len() - 1;
                }
            }
        }
    }
}

pub async fn run(path: Option<PathBuf>) -> Result<()> {
    log4rs::init_file("log4rs.yaml", Default::default())?;
    let stdout = std::io::stdout();
    let (action_channel_tx, action_channel_rx) = tokio::sync::mpsc::channel(100);
    let (alart_channel_tx, alart_channel_rx) = tokio::sync::mpsc::channel(100);
    let rawsize = terminal::size().unwrap();
    let size = Size {
        width: rawsize.0,
        height: rawsize.1,
    };
    let setting: Setting = serde_json::from_reader(std::fs::File::open("settings/default.json")?)?;
    let mut buffer_size = size;
    buffer_size.height -= 2;
    let tabs: Vec<Tab> = match path {
        Some(p) => {
            if p.is_dir() {
                vec![Tab::Directory(directory::Directory::new(p, Pos{row: 1, col: 0}, size, 0)?)]
            } else {
                vec![Tab::Buffer(Buffer::from_file(buffer_size, Pos{row: 1, col: 0}, &p, setting.clone(), 0)?)]
            }
        }
        None => vec![Tab::Buffer(Buffer::new(buffer_size, Pos{row: 1, col: 0}, setting.clone(), 0))]
    };
    let tabs = Arc::new(Mutex::new(tabs));
    let state = Arc::new(Mutex::new(KeymapState::Normal));
    let running = Arc::new(Mutex::new(true));
    let line_input = Arc::new(Mutex::new(lineinput::LineInput::new(size.width as usize)));
    let editor= EditorInfo {
        size,
        setting,
        state,
        running,
        alart_tx: alart_channel_tx,
        tabs,
        line_input,
    };

    let mut event_handler = EventHandler::new(action_channel_tx, editor.clone());
    let mut renderer = Renderer::new(editor.clone(), Box::new(stdout), alart_channel_rx);

    renderer.init().unwrap();
    
    tokio::spawn(async move {
        event_handler.run().await.unwrap();
    });

    tokio::spawn(async move {
        process_action(action_channel_rx, editor.clone(), tabs).await;
    });

    loop {
        let running = editor.running.lock().await;
        if *running == false {
            break;
        }
        renderer.render::<io::Stdout>(0, false).await.unwrap();
    }
    

    renderer.close().unwrap();
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Action {
    pub name: String,
    pub args: Vec<Option<String>>,
}

pub fn parse_action(action: &str, line: &str, idx: usize) -> Result<Action> {
    let r = Regex::new(r"^(\w+)(\((.+)\))?$").unwrap();
    let name = String::from(match r.captures(&action) {
        Some(c) => match c.get(1) {
            Some(c) => c.as_str(),
            None => Err(anyhow::anyhow!("parse_action: invalid action"))?,
        },
        None => Err(anyhow::anyhow!("parse_action: invalid action"))?,
    });
    let args = match r.captures(&action).unwrap().get(3) {
        Some(c) => {
            let args = c.as_str();
            let mut args: Vec<_> = args.split(',').map(|s| Some(String::from(s))).collect();
            for a in args.iter_mut() {
                if let Some(s) = a {
                    match s.as_str() {
                        "$line" => {
                            if line.len() == 0 {
                                *a = None;
                            } else {
                                *a = Some(String::from(line));
                            }
                        }
                        "$idx" => {
                            *s = idx.to_string();
                        }
                        _ => {}
                    }
                };
            }
            args
        },
        None => {
            Vec::new()
        }
    };
    let action = Action {
        name,
        args,
    };
    Ok(action)
}

pub fn syncol_to_crosscol(color: syntect::highlighting::Color) -> crossterm::style::Color {
    crossterm::style::Color::Rgb {
        r: color.r,
        g: color.g,
        b: color.b,
    }
}
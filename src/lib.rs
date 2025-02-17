use std::{collections::HashMap, hash::Hash, sync::Arc};

use anyhow::{Result, Error};
use buffer::{Buffer, Pos};
use key::{open_keymaps, Keymap};
use crossterm::{event::{self, EventStream}, terminal};
use log::debug;
use regex::Regex;
use render::Renderer;
use strum_macros::IntoStaticStr;
use tab::Tab;
use tokio::sync::{mpsc::{self, Receiver}, Mutex};
use tokio_stream::StreamExt;
use serde::{Deserialize, Serialize};

pub mod key;
pub mod buffer;
pub mod render;
pub mod actions;
pub mod tab;
pub mod lineinput;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum TabType {
    Spaces,
    Tabs,
}

#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Setting {
    line_numbers: bool,
    tab_size: usize,
    tab_type: TabType,
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
}

async fn process_action(
    mut action_rx: Receiver<String>, 
    editor: EditorInfo,
    renderer: &mut Renderer,
    mut tabs : Vec<Box<dyn Tab>>,
) {
    type F = Box<dyn FnMut(&Action) -> Vec<actions::ActionReturn>>;
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
    
    renderer.render(&mut tabs, tab_idx, clear).await.unwrap();
    loop {
        if continued {
            renderer.render(&mut tabs, tab_idx, clear).await.unwrap();
            continued = false;
        }
        let action = if let Some(a) = pre_selected_action {
            pre_selected_action = None;
            a
        } else {
            let action = action_rx.recv().await.unwrap();
            parse_action(&action, &renderer.line_input.text).unwrap()
        };
        let mut state = editor.state.lock().await;
        let mut running = editor.running.lock().await;
        let func = action_map.get_mut(action.name.as_str());
        let mut return_queue = Vec::new();
        if let Some(f) = func {
            let returns = f(&action);
            return_queue.extend(returns);
        };
        return_queue.extend(tabs[tab_idx].process_action(&action).unwrap());
        return_queue.push(renderer.line_input.process_action(&action).unwrap());
        for r in return_queue {
            match r {
                actions::ActionReturn::Good => (),
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
                    size.height -= 1;
                    match path {
                        Some(path) => {
                            let new_buffer = match Buffer::from_file(size, Pos{row: 1, col: 0}, &path, editor.setting.line_numbers) {
                                Ok(b) => b,
                                Err(e) => {
                                    editor.alart_tx.send(e).await.unwrap();
                                    continue;
                                }
                            };
                            tabs.push(Box::new(new_buffer));
                        }
                        None => {
                            let new_buffer = Buffer::new(size, Pos{row: 1, col: 0}, editor.setting.line_numbers);
                            tabs.push(Box::new(new_buffer));
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
                    tab_idx = ((tab_idx as isize + i) % len) as usize;
                }
            }
        }
        drop(state);
        renderer.render(&mut tabs, tab_idx, clear).await.unwrap();
    }
}

pub async fn run() -> Result<()> {
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
    let tabs: Vec<Box<dyn Tab>> = vec![Box::new(Buffer::new(buffer_size, Pos{row: 1, col: 0}, setting.line_numbers))];
    let state = Arc::new(Mutex::new(KeymapState::Normal));
    let running = Arc::new(Mutex::new(true));
    let editor= EditorInfo {
        size,
        setting,
        state,
        running,
        alart_tx: alart_channel_tx,
    };

    let mut event_handler = EventHandler::new(action_channel_tx, editor.clone());
    let mut renderer = Renderer::new(editor.clone(), Box::new(stdout), alart_channel_rx);

    renderer.init().unwrap();
    
    tokio::spawn(async move {
        event_handler.run().await.unwrap();
    });

    process_action(action_channel_rx, editor.clone(), &mut renderer, tabs).await;

    renderer.close().unwrap();    
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Action {
    pub name: String,
    pub args: Vec<Option<String>>,
}

pub fn parse_action(action: &str, line: &str) -> Result<Action> {
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
                    if s == "$line" {
                        if line.is_empty() {
                            *a = None;
                        } else {
                            *a = Some(String::from(line));
                        }
                    }
                }
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
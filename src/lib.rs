use std::{collections::HashMap, hash::Hash, sync::Arc};

use anyhow::{Result, Error};
use buffer::{Buffer, Pos};
use key::{open_keymaps, Keymap};
use log::debug;
use crossterm::{event::{self, EventStream}, terminal};
use render::Renderer;
use ropey::Rope;
use strum_macros::IntoStaticStr;
use tokio::sync::{mpsc::{self, Receiver}, Mutex};
use tokio_stream::StreamExt;
use serde::{Deserialize, Serialize};

pub mod key;
pub mod buffer;
pub mod render;

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


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Action {
    Insert(#[serde(skip)] char),
    InsertUpper(#[serde(skip)] char),
    Space,
    Tab,
    NewLine,
    NewLineBelow,
    NewLineAbove,
    Delete,
    DeleteBackward,
    CursorUp,
    CursorDown,
    CursorForward,
    CursorBackward,
    CursorStart,
    CursorEnd,
    NextWord,
    PreviousWord,
    NextBlock,
    PreviousBlock,
    Copy,
    Cut,
    Paste,
    Undo,
    Redo,
    Quit,
    Cmd,
    Find,
    Save,
    SaveAs,
    Open,
    StartOfText,
    EndOfText,
    Normal,
    FindNext,
    FindPrevious,
    LineMode,
    LineInsert(#[serde(skip)] char),
    LineInsertUpper(#[serde(skip)] char),
    LineSpace,
    LineDelete,
    LineDeleteBackward,
    LineCursorForward,
    LineCursorBackward,
    LinePrevious,
    LineNext,
    LineStart,
    LineEnd,
    LineExecute,
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
    action_channel_tx: tokio::sync::mpsc::Sender<Action>,
    keymaps: HashMap<KeymapState, Keymap>,
    reader: EventStream,
    editor: EditorInfo,
}

impl EventHandler {
    pub fn new(action_channel_tx: tokio::sync::mpsc::Sender<Action>, editor: EditorInfo) -> Self 
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
            debug!("Event: {:?}", event);
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
    pub buffers: Arc<Mutex<Vec<Buffer>>>,
    pub state: Arc<Mutex<KeymapState>>,
    pub running: Arc<Mutex<bool>>,
    pub alart_tx: mpsc::Sender<Error>,
}

async fn process_action<W>(
    mut action_rx: Receiver<Action>, 
    editor: EditorInfo,
    renderer: &mut Renderer<W>
) -> Option<Action>
    where W: std::io::Write
{
    let mut buffer_idx = 0;
    let mut continued = false;
    renderer.render(buffer_idx).await.unwrap();
    loop {
        if continued {
            renderer.render(buffer_idx).await.unwrap();
            continued = false;
        }
        debug!("Processing action");
        let action = action_rx.recv().await.unwrap();
        let mut buffers = editor.buffers.lock().await;
        let mut state = editor.state.lock().await;
        let mut running = editor.running.lock().await;
        match action {
            Action::Quit => {
                *running = false;
                return None;
            }
            Action::Insert(c) => {
                buffers[buffer_idx].insert_char(c, false);
            }
            Action::InsertUpper(c) => {
                buffers[buffer_idx].insert_char(c, true);
            }
            Action::Space => {
                buffers[buffer_idx].insert_char(' ', false);
            }
            Action::CursorUp => {
                buffers[buffer_idx].cursor_up();
            }
            Action::CursorDown => {
                buffers[buffer_idx].cursor_down();
            }
            Action::CursorForward => {
                buffers[buffer_idx].cursor_forward();
            }
            Action::CursorBackward => {
                buffers[buffer_idx].cursor_backward();
            }
            Action::CursorStart => {
                buffers[buffer_idx].cursor_start();
            }
            Action::CursorEnd => {
                buffers[buffer_idx].cursor_end();
            }
            Action::NewLine => {
                buffers[buffer_idx].insert_newline();
            }
            Action::Delete => {
                buffers[buffer_idx].delete();
            }
            Action::DeleteBackward => {
                buffers[buffer_idx].delete_back();
            }
            Action::Cmd => {
                *state = KeymapState::Cmd;
            }
            Action::Open => {
                let path = renderer.line_buffer.text.clone();
                let path = path.trim();
                let mut size = editor.size;
                let pos = Pos{row: 0, col: 0};
                size.height -= 1;
                let data = match Buffer::from_file(size, pos, path){
                    Ok(file) => {
                        file
                    }
                    Err(e) => {
                        editor.alart_tx.send(e).await.unwrap();
                        continued = true;
                        continue;
                    }
                };
                buffers.push(data);
                buffer_idx = buffers.len() - 1;
            }
            Action::Normal => {
                *state = KeymapState::Normal;
            }
            Action::Find => {
                *state = KeymapState::Find;
            }
            Action::LineMode => {
                *state = KeymapState::LineInsert;
            }
            Action:: LineInsert(c) => {
                renderer.line_buffer.insert_char(c, false);
            }
            Action::LineInsertUpper(c) => {
                renderer.line_buffer.insert_char(c, true);
            }
            Action::LineSpace => {
                renderer.line_buffer.insert_char(' ', false);
            }
            Action::LineCursorForward => {
                renderer.line_buffer.cursor_forward();
            }
            Action::LineCursorBackward => {
                renderer.line_buffer.cursor_backward();
            }
            Action::LineStart => {
                renderer.line_buffer.cursor_start();
            }
            Action::LineEnd => {
                renderer.line_buffer.cursor_end();
            }
            Action::LineDelete => {
                renderer.line_buffer.delete();
            }
            Action::LineDeleteBackward => {
                renderer.line_buffer.delete_back();
            }
            Action::PreviousBlock => {
            }
            Action::NextBlock => {
            }
            a => {
                debug!("Action: {:?}", a);
            }
        };
        drop(buffers);
        drop(state);
        renderer.render(buffer_idx).await.unwrap();
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
    let setting = serde_json::from_reader(std::fs::File::open("settings/default.json")?)?;
    let mut buffer_size = size;
    buffer_size.height -= 1;
    let buffers = Arc::new(Mutex::new(vec![Buffer::new(buffer_size, Pos{row: 0, col: 0})]));
    let state = Arc::new(Mutex::new(KeymapState::Normal));
    let running = Arc::new(Mutex::new(true));
    let editor= EditorInfo {
        size,
        setting,
        buffers,
        state,
        running,
        alart_tx: alart_channel_tx,
    };

    let mut event_handler = EventHandler::new(action_channel_tx, editor.clone());
    let mut renderer = Renderer::new(editor.clone(), stdout, alart_channel_rx);

    renderer.init().unwrap();
    
    tokio::spawn(async move {
        event_handler.run().await.unwrap();
    });

    process_action(action_channel_rx, editor.clone(), &mut renderer).await;

    renderer.close().unwrap();    
    Ok(())
}
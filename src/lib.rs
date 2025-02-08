use anyhow::Result;
use buffer::Buffer;
use key::{open_keymaps, Keymap};
use log::debug;
use crossterm::{cursor, event::{self, EventStream, KeyboardEnhancementFlags, PushKeyboardEnhancementFlags}, execute, queue, style::{self, Print}, terminal::{self, Clear}};
use strum_macros::IntoStaticStr;
use tokio_stream::StreamExt;
use serde::{Serialize,Deserialize};

pub mod key;
pub mod buffer;

#[derive(Debug, Serialize, Deserialize)]
enum TabType {
    Spaces,
    Tabs,
}

#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Serialize, Deserialize)]
struct Setting {
    line_numbers: bool,
    tab_size: usize,
    tab_type: TabType,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Action {
    Insert(#[serde(skip)] char),
    InsertUpper(#[serde(skip)] char),
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
    Open,
    StartOfText,
    EndOfText,
    ExitCmd,
    FindNext,
    FindPrevious,
    ExitFind,
}


#[derive(Debug, IntoStaticStr, Clone, Copy)]
enum KeymapState {
    Normal,
    Cmd,
    Find,
} 

pub struct Editor<'w, W> 
    where W: std::io::Write
{
    buffers: Vec<Buffer>,
    write: &'w mut W,
    size: Size,
    keymap_state: KeymapState,
    setting: Setting,
}

impl<'w, W> Editor<'w, W> 
    where W: std::io::Write
{
    pub fn new(w: &'w mut W) -> Self 
        where W: std::io::Write
    {
        let rawsize = terminal::size().unwrap();
        let size = Size {
            width: rawsize.0,
            height: rawsize.1,
        };
        Self {
            buffers: vec![Buffer::new(size)],
            write: w,
            size,
            keymap_state: KeymapState::Normal,
            setting: serde_json::from_reader(std::fs::File::open("settings/default.json").unwrap()).unwrap(),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        terminal::enable_raw_mode()?;
        execute!(
            self.write,
            terminal::Clear(terminal::ClearType::All),
            style::ResetColor,
            event::EnableMouseCapture,
            cursor::Show,
            cursor::MoveTo(0, 0),
        )?;
        queue!(
            self.write,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES |
                KeyboardEnhancementFlags::REPORT_EVENT_TYPES |
                KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
            )
        )?;
        execute!(self.write, cursor::MoveTo(0, 0))?;
        let mut reader = EventStream::new();
        let keymaps = open_keymaps("settings/keymap.json")?;
        'exit:
        while let Some(event) = reader.next().await {
            let keymap = keymaps.get(self.keymap_state.into()).unwrap();
            if let Ok(event) = event {
                match event {
                    event::Event::Key(event) => {
                        let key = Keymap::read(event);
                        if let Some(key) = key {
                            match keymap.get_action(key) {
                                Some(Action::Quit) => {
                                    break 'exit;
                                }
                                Some(Action::Insert(c)) => {
                                    self.buffers[0].insert_char(c, false);
                                }
                                Some(Action::InsertUpper(c)) => {
                                    self.buffers[0].insert_char(c, true);
                                }
                                Some(Action::CursorUp) => {
                                    self.buffers[0].cursor_up();
                                }
                                Some(Action::CursorDown) => {
                                    self.buffers[0].cursor_down();
                                }
                                Some(Action::CursorForward) => {
                                    self.buffers[0].cursor_forward();
                                }
                                Some(Action::CursorBackward) => {
                                    self.buffers[0].cursor_backward();
                                }
                                Some(Action::CursorStart) => {
                                    self.buffers[0].cursor_start();
                                }
                                Some(Action::CursorEnd) => {
                                    self.buffers[0].cursor_end();
                                }
                                Some(Action::NewLine) => {
                                    self.buffers[0].insert_newline();
                                }
                                Some(Action::Delete) => {
                                    self.buffers[0].delete();
                                }
                                Some(Action::DeleteBackward) => {
                                    self.buffers[0].delete_back();
                                }
                                Some(Action::Cmd) => {
                                    self.keymap_state = KeymapState::Cmd;
                                }
                                Some(Action::ExitCmd) => {
                                    self.keymap_state = KeymapState::Normal;
                                }
                                Some(Action::Find) => {
                                    self.keymap_state = KeymapState::Find;
                                }
                                Some(Action::ExitFind) => {
                                    self.keymap_state = KeymapState::Normal;
                                }
                                Some(Action::PreviousBlock) => {
                                }
                                Some(Action::NextBlock) => {
                                }
                                Some(a) => {
                                    debug!("Action: {:?}", a);
                                }
                                None => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            self.render()?;
        }
        execute!(
            self.write,
            event::DisableMouseCapture,
            terminal::Clear(terminal::ClearType::All),
        )?;
        terminal::disable_raw_mode()?;

        Ok(())
    }

    fn render(&mut self) -> Result<()> {
        execute!(self.write, cursor::Hide)?;
        let scroll = self.buffers[0].scroll;
        let mut lines = self.buffers[0].text.lines().skip(scroll);
        let line_len = self.buffers[0].text.len_lines();
        let line_num_padding = if self.setting.line_numbers {
            numlen(line_len) + 1
        } else {
            0
        };
        for i in scroll..(self.size.height as usize + scroll) {
            if self.setting.line_numbers {
                queue!(
                    self.write,
                    cursor::MoveTo(0, i as u16 - scroll as u16),
                )?;
                execute!(
                    self.write,
                    Print(format!("{: >1$} ", i + 1, line_num_padding - 1)),
                )?;
            }
            queue!(
                self.write,
                cursor::MoveTo(line_num_padding as u16, i as u16 - scroll as u16),
            )?;
            if i < line_len {
                let mut to_print = lines.next().unwrap();
                if to_print.chars().last().unwrap_or('\0') == '\n' {
                    to_print = to_print.slice(..to_print.len_chars() - 1);
                }
                execute!(
                    self.write,
                    Print(to_print),
                    Clear(terminal::ClearType::UntilNewLine),
                )?;
            } else {
                execute!(
                    self.write,
                    Clear(terminal::ClearType::CurrentLine),
                )?;
            }
        }
        execute!(self.write, cursor::Show)?;
        execute!(
            self.write,
            cursor::MoveTo(
                self.buffers[0].cursor.col as u16 + line_num_padding as u16,
                self.buffers[0].cursor.row as u16 - scroll as u16,
            ),
        )?;
        Ok(())
    }

}

fn numlen (mut num: usize) -> usize {
    let mut len = 0;
    while num > 0 {
        num /= 10;
        len += 1;
    }
    len
}
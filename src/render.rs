use std::{io::Write, sync::Arc};

use anyhow::{Error, Result};
use crossterm::{cursor, execute, queue, style::{self, Colors, Print, StyledContent, Stylize}, terminal::{self, EnterAlternateScreen, LeaveAlternateScreen}};
use log::error;
use tokio::sync::{mpsc, Mutex};

use crate::{lineinput::LineInput, syncol_to_crosscol, tab::Tab, EditorInfo, KeymapState};

#[derive(Debug)]
pub struct Renderer<W>
where
    W: Write,
{
    editor: EditorInfo,
    write: W,
    alart_rx: mpsc::Receiver<Error>,
}

impl<W> Renderer<W>
where
    W: Write,
{
    pub fn new(editor: EditorInfo, w: W, alart_rx: mpsc::Receiver<Error>) -> Self 
    {
        Self {
            editor,
            write: w,
            alart_rx,
        }
    }

    pub async fn render<T>(&mut self, idx: usize, clear: bool) -> Result<()> 
    where
    {
        let state = self.editor.state.lock().await;
        let line_input = self.editor.line_input.lock().await;
        let tabs = self.editor.tabs.lock().await;
        let cursor = match tabs[idx] {
            Tab::Buffer(ref buffer) => buffer.get_cursor(),
            Tab::Directory(ref directory) => directory.get_cursor(),
            Tab::Shell(ref shell) => shell.get_cursor(),
        };
        if clear {
            queue!(self.write, terminal::Clear(terminal::ClearType::All))?;
        }
        queue!(
            self.write,
            cursor::Hide,
            cursor::MoveTo(0, 0),
        )?;
        match tabs[idx]{ 
            Tab::Buffer(ref mut buffer) => {
                buffer.render(&mut self.write)?;
            }
            Tab::Directory(ref mut directory) => {
                directory.render(&mut self.write)?;
            }
            Tab::Shell(ref mut shell) => {
                shell.render(&mut self.write).await?;
            }
        }
        // Render the tab bar
        let mut tab_bar = Bar::new(self.editor.size.width as usize, 0);
        let tab_ratio = if 1.0 / tabs.len() as f32 > 0.3 {
            1.0 / tabs.len() as f32
        } else {
            0.3
        };
        for (i, tab) in tabs.iter().enumerate() {
            let name = match tab {
                Tab::Buffer(buffer) => buffer.name(),
                Tab::Directory(directory) => directory.name(),
                Tab::Shell(shell) => shell.name(),
            };
            let s = name.clone();
            let s = if i == idx {
                s.bold().reverse()
            } else {
                s.bold()
            };
            tab_bar.add(s, tab_ratio as f32 * (i as f32), name.len());
        }
        tab_bar.render(&mut self.write)?;

        // Render the status bar
        let mut status_bar = Bar::new(self.editor.size.width as usize, self.editor.size.height as usize - 1);
        let mut lineinput_cur= 0;
        let mut lineinput_pos= 0;
        if let Ok(e) = self.alart_rx.try_recv() {
            let s = format!("Alart: {}", e.to_string());
            status_bar.add(s.clone().red(), 0.0, s.len());
            error!("Alart: {}", e.to_string());
        } else {
            let keystate_str: &'static str = (*state).into();
            let keystate_str = format!("State: {}", keystate_str);
            let line = format!("{}{}",line_input.notice, line_input.text);
            lineinput_cur = line_input.cur + line_input.notice.len();
            status_bar.background = " ".reverse();
            status_bar.add(keystate_str.clone().reverse(), 0.0, keystate_str.len());
            lineinput_pos = status_bar.add(line.clone().white(), 0.2, line.len());
        }
        status_bar.render(&mut self.write)?;
        // End of rendering
        if *state == KeymapState::LineInsert {
            execute!(
                self.write,
                cursor::Show,
                cursor::MoveTo(
                    lineinput_pos as u16 + lineinput_cur as u16,
                    self.editor.size.height - 1,
                ),
            )?;
        } else {
            match cursor {
                Some(cursor) => {
                    execute!(
                        self.write,
                        cursor::Show,
                        cursor::MoveTo(
                            cursor.col as u16,
                            cursor.row as u16,
                        ),
                    )?;
                }
                None => {
                    execute!(
                        self.write,
                        cursor::Hide,
                    )?;
                }
            }
        }
        Ok(())
    }

    pub fn init(&mut self) -> Result<()> 
    {
        execute!(
            self.write,
            EnterAlternateScreen,
        )?;
        terminal::enable_raw_mode()?;
        execute!(
            self.write,
            terminal::Clear(terminal::ClearType::All),
            cursor::Show,
            cursor::MoveTo(0, 0),
        )?;
        /*queue!(
            self.write,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES |
                KeyboardEnhancementFlags::REPORT_EVENT_TYPES |
                KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
            )
        )?;*/
        Ok(())
    }

    pub fn close (&mut self) -> Result<()> {
        /*queue!(
            self.write,
            PopKeyboardEnhancementFlags,
        )?;*/
        queue!(
            self.write,
            terminal::Clear(terminal::ClearType::All),
            cursor::Show,
        )?;
        terminal::disable_raw_mode()?;
        execute!(
            self.write,
            LeaveAlternateScreen,
        )?;
        Ok(())
    }
}

struct Bar {
    len: usize,
    row: usize,
    strings: Vec<(StyledContent<String>, f32, usize)>,
    pub background: StyledContent<&'static str>,
}

impl Bar {
    fn new(len: usize, row: usize) -> Self {
        Self {
            len,
            row,
            strings: Vec::new(),
            background: " ".on_black(),
        }
    }

    fn add(&mut self, s: StyledContent<String>, ratio: f32, len: usize) -> usize {
        self.strings.push((s, ratio, len));
        (self.len as f32 * ratio) as usize
    }

    fn render<W>(&self, write: &mut W) -> Result<()> 
    where W: Write
    {
        let mut bar = vec![true; self.len];
        let strings = self.strings.iter().map(|(s, r, l)| {
            let pos = (self.len as f32 * r) as usize;
            (s.clone(), pos, l)
        }).collect::<Vec<_>>();
        for (s, pos, len) in strings {
            queue!(write, cursor::MoveTo(pos as u16, self.row as u16), Print(s))?;
            for i in pos..pos + len {
                if i < self.len { bar[i] = false; }
            }
        }
        for i in 0..self.len {
            if bar[i] {
                queue!(write, cursor::MoveTo(i as u16, self.row as u16), Print(&self.background))?;
            }
        }
        Ok(())
    }
}
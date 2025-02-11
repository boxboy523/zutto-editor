use anyhow::{Result, Error};
use crossterm::{cursor, event::{self, KeyboardEnhancementFlags, PushKeyboardEnhancementFlags}, execute, queue, style::{self, Print, Stylize}, terminal::{self, Clear}};
use log::debug;
use serde::{de::Visitor, Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};

use crate::{buffer::{Buffer, BufferLine, Pos}, EditorInfo, KeymapState, Setting, Size};

fn numlen (mut num: usize) -> usize {
    let mut len = 0;
    while num > 0 {
        num /= 10;
        len += 1;
    }
    len
}

#[derive(Debug)]
pub struct Renderer<W> 
    where W: std::io::Write
{
    editor: EditorInfo,
    write: W,
    alart_rx: mpsc::Receiver<Error>,
    pub line_buffer: BufferLine,
} 

impl<W> Renderer<W> 
    where W: std::io::Write
{
    pub fn new(editor: EditorInfo, w: W, alart_rx: mpsc::Receiver<Error>) -> Self 
    {
        let line_buffer = BufferLine::new(
            Pos{row: editor.size.height as usize - 1, col: (editor.size.width / 2) as usize},
            (editor.size.width / 2 - 1) as usize,
        );
        Self {
            editor,
            write: w,
            alart_rx,
            line_buffer,
        }
    }

    pub async fn render(&mut self, idx: usize) -> Result<()> {
        debug!("Rendering");
        let state = self.editor.state.lock().await;
        let buffers = self.editor.buffers.lock().await;
        let buffer = &buffers[idx];
        let camera = buffer.camera;
        let line_len = buffer.text.len_lines();
        let cursor = buffer.cursor;
        let mut lines = buffer.text.lines().skip(camera.row);
        execute!(
            self.write,
            cursor::Hide,
            style::ResetColor,
        )?;
        
        let line_num_padding = if self.editor.setting.line_numbers {
            numlen(line_len) + 1
        } else {
            0
        };
        for i in camera.row..(self.editor.size.height as usize + camera.row) {
            if self.editor.setting.line_numbers {
                queue!(
                    self.write,
                    cursor::MoveTo(0, i as u16 - camera.row as u16),
                )?;
                execute!(
                    self.write,
                    Print(format!("{: >1$} ", i + 1, line_num_padding - 1)),
                )?;
            }
            queue!(
                self.write,
                cursor::MoveTo(line_num_padding as u16, i as u16 - camera.row as u16),
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
        if let Ok(e) = self.alart_rx.try_recv() {
            execute!(
                self.write,
                cursor::MoveTo(0, self.editor.size.height - 1),
                Print(format!("Alart: {}", e.to_string()).red()),
            )?;
        } else {
            let keystate_str: &'static str = (*state).into();
            execute!(
                self.write,
                cursor::MoveTo(0, self.editor.size.height - 1),
                Print(format!("State: {}", keystate_str).reverse()),
                cursor::MoveTo(
                    self.line_buffer.pos.col as u16,
                    self.editor.size.height - 1,
                ),
                Print(&self.line_buffer.text),
            )?;
        }
        // End of rendering
        execute!(self.write, cursor::Show)?;
        if *state == KeymapState::LineInsert {
            execute!(
                self.write,
                cursor::MoveTo(
                    self.line_buffer.pos.col as u16 + self.line_buffer.cur as u16,
                    self.editor.size.height - 1,
                ),
            )?;
        } else {
        execute!(
                self.write,
                cursor::MoveTo(
                    cursor.col as u16 + line_num_padding as u16,
                    cursor.row as u16 - camera.row as u16,
                ),
            )?;
        }
        Ok(())
    }

    pub fn init(&mut self) -> Result<()> 
    {
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
        Ok(())
    }

    pub fn close (&mut self) -> Result<()> {
        execute!(
            self.write,
            terminal::Clear(terminal::ClearType::All),
            cursor::Show,
            event::DisableMouseCapture,
        )?;
        terminal::disable_raw_mode()?;
        Ok(())
    }
}

use std::{io::Write, path};

use anyhow::Result;
use crossterm::{cursor, execute, style::Print, terminal::{self, Clear}};
use log::debug;
use ropey::Rope;

use crate::{actions::ActionReturn, tab::{numlen, Tab}, Action, KeymapState, Size};


fn len_no_newline(rope: &Rope, line: usize) -> usize {
    let line_slice = rope.line(line);
    if line_slice.len_chars() == 0 {
        return 0;
    } else
    if line_slice.char(line_slice.len_chars() - 1) == '\n' {
        return line_slice.len_chars() - 1;
    } else {
        return line_slice.len_chars();
    }
}

fn end_of_line(rope: &Rope, line: usize) -> usize {
    let line_slice = rope.line(line);
    if line_slice.len_chars() == 0 {
        return 0;
    } else
    if line_slice.char(line_slice.len_chars() - 1) == '\n' {
        return line_slice.len_chars() - 1;
    } else {
        return line_slice.len_chars();
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub row: usize,
    pub col: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct Pos {
    pub row: usize,
    pub col: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct Cursor {
    pub row: usize,
    pub col: usize,
}

#[derive(Debug)]
pub struct Buffer {
    text: Rope,
    cursor: Cursor,
    camera: Camera,
    size: Size,
    pos: Pos,
    path: Option<String>, //None if it is a new buffer
    line_num: bool,
}

impl Buffer {
    pub fn new(size: Size, pos: Pos, line_num: bool) -> Self {
        Self {
            pos,
            text: Rope::new(),
            cursor: Cursor {
                row: 0,
                col: 0,
            },
            camera: Camera {
                row: 0,
                col: 0,
            },
            size,
            path: None,
            line_num,
        }
    }

    pub fn resize(&mut self, size: Size) {
        self.size = size;
        if self.cursor.row >= self.text.len_lines() {
            self.cursor.row = self.text.len_lines() - 1;
        }
        if self.cursor.col > len_no_newline(&self.text, self.cursor.row) {
            self.cursor.col = len_no_newline(&self.text, self.cursor.row);
        }
    }

    pub fn from_file(size: Size, pos: Pos, path: &str, line_num: bool) -> Result<Self> {
        let text = Rope::from_reader(std::fs::File::open(path)?)?;
        Ok(Self {
            text,
            cursor: Cursor { row: 0, col: 0 },
            camera: Camera { row: 0, col: 0 },
            size,
            pos,
            path: Some(path.to_string()), 
            line_num,
        })
    }

    pub fn save(&self, p: Option<&str>) -> Result<()> {
        let byte: Vec<u8> = self.text.bytes().collect();
        if let Some(path) = p {
            let mut file = std::fs::File::create(path)?; 
            file.write_all(&byte)?;
        } else if let Some(path) = &self.path {
            let mut file = std::fs::File::create(path)?;
            file.write_all(&byte)?;
        } else {
            return Err(anyhow::anyhow!("No path to save, use save_as(Cmd: Ctrl+S)"));
        }
        Ok(())
    }

    pub fn cursor_up(&mut self) {
        if self.cursor.row > 0 {
            self.cursor.row -= 1;
            if self.cursor.col > len_no_newline(&self.text, self.cursor.row) {
                self.cursor.col = self.text.line(self.cursor.row).len_chars() - 1;
            }
        }
        if self.cursor.row < self.camera.row {
            self.camera.row -= 1;
        }
    }

    pub fn cursor_down(&mut self) {
        if self.text.len_lines() > 0 && self.cursor.row < self.text.len_lines() - 1  {
            self.cursor.row += 1;
            if self.cursor.col > len_no_newline(&self.text, self.cursor.row) {
                self.cursor.col = len_no_newline(&self.text, self.cursor.row);
            }
        }
        if self.cursor.row > self.camera.row + self.size.height as usize - 1 {
            self.camera.row += 1;
        }
    }

    pub fn cursor_forward(&mut self) {
        if self.cursor.col < len_no_newline(&self.text, self.cursor.row) {
            self.cursor.col += 1;
            if self.cursor.col > self.camera.col + self.size.width as usize {
                self.camera.col += 1;
            }
        } else if self.cursor.row < self.text.len_lines() - 1 {
            self.cursor.row += 1;
            self.cursor.col = 0;
        }
    }

    pub fn cursor_backward(&mut self) {
        if self.cursor.col > 0 {
            self.cursor.col -= 1;
            if self.cursor.col < self.camera.col {
                self.camera.col -= 1;
            }
        } else if self.cursor.row > 0 {
            self.cursor.row -= 1;
            self.cursor.col = len_no_newline(&self.text, self.cursor.row);
        }
    }

    pub fn cursor_start(&mut self) {
        self.cursor.col = 0;
    }

    pub fn cursor_end(&mut self) {
        self.cursor.col = end_of_line(&self.text, self.cursor.row);
    }

    fn get_idx(&self) -> usize {
        self.text.line_to_char(self.cursor.row) + self.cursor.col
    }

    pub fn insert_char(&mut self, c: char, upper: bool) {
        let c = if upper { c } else { c.to_lowercase().next().unwrap() };
        self.text.insert_char(self.get_idx(), c);
        self.cursor_forward();
    }

    pub fn insert_str(&mut self, s: &str) {
        self.text.insert(self.get_idx(), s);
        self.cursor_forward();
    }

    pub fn insert_newline(&mut self) {
        self.text.insert_char(self.get_idx(), '\n');
        self.cursor_down();
        self.cursor_start();
    }

    pub fn insert_newline_above(&mut self) {
        if self.cursor.row > 0 {
            let idx = self.text.line_to_char(self.cursor.row - 1) + len_no_newline(&self.text, self.cursor.row - 1);
            self.text.insert_char(idx, '\n');
            self.cursor_down();
        } else {
            self.insert_newline();
        }
    }

    pub fn insert_newline_below(&mut self) {
        let idx = self.text.line_to_char(self.cursor.row) + len_no_newline(&self.text, self.cursor.row);
        self.text.insert_char(idx, '\n');
    }

    pub fn delete(&mut self) {
        if self.text.len_chars() > 0  && self.get_idx() != 0 {
            let c = self.text.char(self.get_idx() - 1);
            self.text.remove(self.get_idx() - 1..self.get_idx());
            if c == '\n' {
                self.cursor_up();
                self.cursor_end();
            } else {
                self.cursor_backward();
            }
        }
    }

    pub fn delete_back(&mut self) {
        if self.text.len_chars() > 0 && self.get_idx() < self.text.len_chars() {
            self.text.remove(self.get_idx()..self.get_idx() + 1);
        }
    }
}

impl Tab for Buffer {
    fn render(&self, write: &mut Box<dyn Write>) -> Result<()>
    {
        let camera = self.camera;
        let line_len = self.text.len_lines();
        let mut lines = self.text.lines().skip(camera.row);
        let line_num_padding = if self.line_num {
            numlen(line_len) + 1
        } else {
            0
        };
        debug!("pos: {:?}, camera: {:?}, cursor: {:?}", self.pos, camera, self.cursor);
        for i in 0..self.size.height as usize {
            let line = match lines.next() {
                Some(l) => l,
                None => continue,
            };
            let line = if self.line_num {
                format!("{:width$} {}", i + 1 + camera.row, line, width = line_num_padding)
            } else {
                line.to_string()
            };
            execute!(
                write,
                cursor::MoveTo(self.pos.col as u16, i as u16 + self.pos.row as u16),
                Clear(terminal::ClearType::UntilNewLine),
                Print(line),
            )?;
        }
        Ok(())
    }

    fn get_pos(&self) -> Pos {
        self.pos
    }

    fn get_name(&self) -> String {
        match &self.path {
            Some(p) => p.to_string(),
            None => "Untitled".to_string(),
        }
    }

    fn get_size(&self) -> Size {
        self.size
    }

    fn get_cursor(&self) -> Cursor {
        let line_num_padding = if self.line_num {
            numlen(self.text.len_lines()) + 2
        } else {
            0
        };
        let mut cursor = self.cursor;
        cursor.col += line_num_padding - self.camera.col - self.pos.col;
        cursor.row -= self.camera.row;
        cursor.row += self.pos.row;
        cursor
    }
    fn process_action(&mut self, action: &Action) -> Result<Vec<ActionReturn>> {
        let action_name = &action.name;
        let mut action_args = action.args.clone();
        match action_name.as_str() {
            "CursorUp" => { self.cursor_up(); }
            "CursorDown" => { self.cursor_down(); }
            "CursorForward" => {
                self.cursor_forward();
            }
            "CursorBackward" => {
                self.cursor_backward();
            }
            "CursorStart" => {
                self.cursor_start();
            }
            "CursorEnd" => {
                self.cursor_end();
            }
            "Insert" => {
                let c = action_args[0].as_mut().unwrap().chars().next().unwrap();
                self.insert_char(c, false);
            }
            "InsertUpper" => {
                let c = action_args[0].as_mut().unwrap().chars().next().unwrap();
                self.insert_char(c, true);
            }
            "InsertStr" => {
                let s = action_args[0].as_ref().unwrap();
                self.insert_str(s);
            }
            "InsertNewline" => {
                self.insert_newline();
            }
            "InsertNewlineAbove" => {
                self.insert_newline_above();
            }
            "InsertNewlineBelow" => {
                self.insert_newline_below();
            }
            "Delete" => {
                self.delete();
            }
            "DeleteBack" => {
                self.delete_back();
            }
            "Open" => {
                if action_args[0].is_none() {
                    return Ok(vec![
                        ActionReturn::State(KeymapState::LineInsert),
                        ActionReturn::Notice("Enter file name: ".to_string()),
                        ActionReturn::ExcuteLine("Open($line)".to_string()),
                    ]);
                } else {
                    let path = action_args[0].as_ref().unwrap();
                    return Ok(vec![
                        ActionReturn::Notice(format!("Opened {}", path)),
                        ActionReturn::NewBuffer(Some(path.to_string())),
                    ]);
                }
            }
            "Save" => {
                if self.path.is_none() {
                    return Ok(vec![
                        ActionReturn::State(KeymapState::LineInsert),
                        ActionReturn::Notice("Enter file name: ".to_string()),
                        ActionReturn::Excute(Action {
                            name: "SaveAs".to_string(),
                            args: vec![],
                        }),
                    ]);
                }
                match self.save(None) {
                    Ok(_) => {
                        return Ok(vec![ActionReturn::Notice("Saved".to_string())]);
                    }
                    Err(e) => {
                        return Ok(vec![ActionReturn::Err(e)]);
                    }
                }
            }
            "SaveAs" => {
                if action_args.is_empty() {
                    return Ok(vec![
                        ActionReturn::State(KeymapState::LineInsert),
                        ActionReturn::Notice("Enter file name: ".to_string()),
                        ActionReturn::Excute(Action {
                            name: "SaveAs".to_string(),
                            args: vec![],
                        }),
                    ]);
                } else {
                    match self.save(Some(action_args[0].as_ref().unwrap())) {
                        Ok(_) => {
                            return Ok(vec![ActionReturn::Notice("Saved".to_string())]);
                        }
                        Err(e) => {
                            return Ok(vec![ActionReturn::Err(e)]);
                        }
                    }
                }
            }
            _ => (),
        }
        Ok(vec![ActionReturn::Good])    
    }
}
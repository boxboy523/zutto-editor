use std::{cmp::min, io::Write, path::{self, PathBuf}};

use anyhow::Result;
use crossterm::{cursor, execute, queue, style::{Print, Stylize}, terminal::{self, Clear}};
use log::debug;
use ropey::Rope;

use crate::{actions::ActionReturn, Action, KeymapState, Setting};

use super::{numlen, Cursor, Pos, Size, Tab};

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub row: usize,
    pub col: usize,
}

#[derive(Debug)]
pub struct Buffer {
    text: Rope,
    cursor_idx: usize,
    camera: Camera,
    size: Size,
    pos: Pos,
    path: Option<PathBuf>, //None if it is a new buffer
    area_start: Option<usize>,
    setting: Setting,
}

impl Buffer {
    pub fn new(size: Size, pos: Pos, setting: Setting) -> Self {
        Self {
            pos,
            text: Rope::new(),
            cursor_idx: 0,
            camera: Camera {
                row: 0,
                col: 0,
            },
            size,
            path: None,
            area_start: None,
            setting,
        }
    }

    pub fn resize(&mut self, size: Size) {
        self.size = size;
    }

    pub fn from_file(size: Size, pos: Pos, path: &PathBuf, setting: Setting) -> Result<Self> {
        let text = Rope::from_reader(std::fs::File::open(path)?)?;
        Ok(Self {
            text,
            cursor_idx: 0,
            camera: Camera { row: 0, col: 0 },
            size,
            pos,
            path: Some(path.clone()), 
            area_start: None,
            setting
        })
    }

    fn save(&self, p: Option<&str>) -> Result<()> {
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

    fn get_row(&self) -> usize {
        let i = self.cursor_idx;
        let mut row = 0;
        let chars = self.text.chars().take(i);
        for c in chars {
            if c == '\n' {
                row += 1;
            }
        }
        row
    }
    fn get_row_start(&self) -> usize {
        let i = self.cursor_idx;
        let chars  = self.text.chars().take(i);
        let mut start = 0;
        for (j, c) in chars.enumerate() {
            if c == '\n' {
                start = j + 1;
            }
        }
        start
    }
    fn get_row_end(&self) -> usize {
        let i = self.cursor_idx;
        let chars = self.text.chars().skip(i);
        for (j, c) in chars.enumerate() {
            if c == '\n' {
                return i + j;
            }
        }
        self.text.len_chars()
    }

    fn get_row_len(&self) -> usize {
        let i = self.cursor_idx;
        let chars = self.text.chars().skip(i);
        for (j, c) in chars.enumerate() {
            if c == '\n' {
                return j;
            }
        }
        self.text.len_chars() - i
    }
    fn get_col(&self) -> usize {
        let i = self.get_row_start();
        return self.cursor_idx - i;
    }

    fn cursor_up(&mut self) {
        if self.get_row() == 0 {
            return;
        }
        let col = self.get_col();
        self.cursor_idx = self.get_row_start() - 1;
        self.cursor_idx = self.get_row_start();
        self.cursor_idx += min(col, self.get_row_len());
        if self.get_row() < self.camera.row {
            self.camera.row -= 1;
        }
    }

    fn cursor_down(&mut self) {
        if self.get_row() == self.text.len_lines() - 1 {
            return;
        }
        let col = self.get_col();
        self.cursor_idx = self.get_row_end() + 1;
        self.cursor_idx += min(col, self.get_row_len());
        if self.get_row() >= self.camera.row + self.size.height as usize {
            self.camera.row += 1;
        }
    }

    fn cursor_forward(&mut self) {
        if self.cursor_idx < self.text.len_chars() {
            self.cursor_idx += 1;
        }
        if self.get_col() > self.size.width as usize {
            self.camera.col += 1;
        }
        if self.get_row() >= self.camera.row + self.size.height as usize {
            self.camera.row += 1;
        }
    }

    fn cursor_backward(&mut self) {
        if self.cursor_idx > 0 {
            self.cursor_idx -= 1;
        }
        if self.get_col() < self.camera.col {
            self.camera.col -= 1;
        }
        if self.get_row() < self.camera.row {
            self.camera.row -= 1;
        }
    }

    fn cursor_start(&mut self) {
        let i = self.cursor_idx;
        for j in (0..i).rev() {
            if self.text.char(j) == '\n' {
                self.cursor_idx = j + 1;
                return;
            }
        }
    }

    fn cursor_end(&mut self) {
        let i = self.cursor_idx;
        for j in i..self.text.len_chars() {
            if self.text.char(j) == '\n' {
                self.cursor_idx = j;
                return;
            }
        }
    }

    fn cursor_forward_word(&mut self) {
        let i = self.cursor_idx;
        let mut j = i;
        while j < self.text.len_chars() && self.text.char(j).is_whitespace() {
            j += 1;
        }
        while j < self.text.len_chars() && !self.text.char(j).is_whitespace() {
            j += 1;
        }
        self.cursor_idx = j;
        if self.get_col() >= self.camera.col + self.size.width as usize {
            self.camera.col += j - i;
        }
        if self.get_row() >= self.camera.row + self.size.height as usize {
            self.camera.row += 1;
        }
    }

    fn cursor_backward_word(&mut self) {
        let i = self.cursor_idx;
        let mut j = i;
        while j > 0 && self.text.char(j - 1).is_whitespace() {
            j -= 1;
        }
        while j > 0 && !self.text.char(j - 1).is_whitespace() {
            j -= 1;
        }
        self.cursor_idx = j;
        if self.get_col() < self.camera.col {
            self.camera.col -= i - j;
        }
        if self.get_row() < self.camera.row {
            self.camera.row -= 1;
        }
    }

    fn insert_char(&mut self, c: char, upper: bool) {
        let c = if upper { c } else { c.to_lowercase().next().unwrap() };
        self.text.insert_char(self.cursor_idx, c);
        self.cursor_forward();
    }

    fn insert_str(&mut self, s: &str) {
        self.text.insert(self.cursor_idx, s);
        self.cursor_forward();
    }

    fn insert_newline(&mut self) {
        self.text.insert_char(self.cursor_idx, '\n');
        self.cursor_forward();
    }

    fn insert_tab(&mut self) {
        match self.setting.tab_type {
            crate::TabType::Space => {
                let tab_size = self.setting.tab_size;
                for _ in 0..tab_size {
                    self.text.insert_char(self.cursor_idx, ' ');
                    self.cursor_forward();
                }
            }
            crate::TabType::Tab => {
                self.text.insert_char(self.cursor_idx, '\t');
                self.cursor_forward();
            }
        }
    }

    fn insert_newline_above(&mut self) {
        let idx = self.get_row_start();
        self.text.insert_char(idx, '\n');
    }

    fn insert_newline_below(&mut self) {
        let idx = self.get_row_end();
        self.text.insert_char(idx, '\n');
    }

    fn delete(&mut self) {
        if self.text.len_chars() > 0 && self.cursor_idx > 0 {
            if self.text.char(self.cursor_idx - 1) == ' ' && self.cursor_idx % self.setting.tab_size == 0 {
                let mut i = self.cursor_idx - 1;
                let mut t = 0;
                while i > 0 && self.text.char(i) == ' ' && t < self.setting.tab_size - 1 {
                    i -= 1;
                    t += 1;
                }
                self.text.remove(i..self.cursor_idx - 1);
                self.cursor_idx = i;
                return;
            }
            self.text.remove(self.cursor_idx - 1..self.cursor_idx);
            self.cursor_backward();
        }
    }

    fn delete_back(&mut self) {
        if self.text.len_chars() > 0 && self.cursor_idx < self.text.len_chars() {
            self.text.remove(self.cursor_idx..self.cursor_idx + 1);
        }
    }

    fn visualize(&self, line: usize) -> Option<String> {
        let line_slice = match self.text.get_line(line) {
            Some(l) => l,
            None => return None,
        };
        let mut s = line_slice.to_string();
        if line_slice.len_chars() == 0 {
            return Some(s);
        }
        if line_slice.char(line_slice.len_chars() - 1) == '\n' {
            s.pop();
        }
        for (i, c) in line_slice.chars().enumerate() {
            if c == '\t' {
                let tab_size = self.setting.tab_size - i % self.setting.tab_size;
                s.replace_range(i..i+1, " ".repeat(tab_size).as_str());
            }
        }
        Some(s)
    }
}

impl Tab for Buffer {
    fn render(&self, write: &mut Box<dyn Write>) -> Result<()>
    {
        let camera = self.camera;
        let line_len = self.text.len_lines();
        let line_num_padding = if self.setting.line_numbers {
            numlen(line_len) + 1
        } else {
            0
        };
        for i in 0..self.size.height as usize {
            let line = match self.visualize(i + camera.row) {
                Some(l) => l,
                None => {
                    execute!(
                        write,
                        cursor::MoveTo(self.pos.col as u16, i as u16 + self.pos.row as u16),
                        Clear(terminal::ClearType::UntilNewLine),
                    )?;
                    continue;
                },
            };
            queue!(
                write,
                cursor::MoveTo(self.pos.col as u16, i as u16 + self.pos.row as u16),
                Clear(terminal::ClearType::UntilNewLine),
            )?;
            if self.setting.line_numbers {
                let line_num = format!("{:width$} ", i + 1 + camera.row, width = line_num_padding);
                if i + camera.row == self.get_row() {
                    queue!(
                        write,
                        Print(line_num.white()),
                    )?;
                } else {
                    queue!(
                        write,
                        Print(line_num.dark_grey()),
                    )?;
                }
            }
            execute!(
                write, 
                Print(line)
            )?;
        }
        Ok(())
    }

    fn get_pos(&self) -> Pos {
        self.pos
    }

    fn get_name(&self) -> String {
        match &self.path {
            Some(p) => p.file_name().unwrap().to_string_lossy().to_string(),
            None => "Untitled".to_string(),
        }
    }

    fn get_size(&self) -> Size {
        self.size
    }

    fn get_cursor(&self) -> Option<Cursor> {
        let line_num_padding = if self.setting.line_numbers {
            numlen(self.text.len_lines()) + 2
        } else {
            0
        };
        let mut cursor = Cursor {
            row: self.get_row(),
            col: self.get_col(),
        };
        cursor.col += line_num_padding - self.camera.col - self.pos.col;
        cursor.row -= self.camera.row;
        cursor.row += self.pos.row;
        Some(cursor)
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
            "CursorForwardWord" => {
                self.cursor_forward_word();
            }
            "CursorBackwardWord" => {
                self.cursor_backward_word();
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
            "InsertSpace" => {
                self.insert_str(" ");
            }
            "InsertTab" => {
                self.insert_tab();
            }
            "Delete" => {
                self.delete();
            }
            "DeleteBack" => {
                self.delete_back();
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
                        return Ok(vec![
                            ActionReturn::Notice("Saved".to_string()),
                            ActionReturn::State(KeymapState::Normal),
                        ]);
                    }
                    Err(e) => {
                        return Ok(vec![
                            ActionReturn::Err(e),
                            ActionReturn::State(KeymapState::Normal),
                        ]);
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
                            return Ok(vec![
                                ActionReturn::Notice("Saved".to_string()),
                                ActionReturn::State(KeymapState::Normal),
                            ]);
                        }
                        Err(e) => {
                            return Ok(vec![
                                ActionReturn::Err(e),
                                ActionReturn::State(KeymapState::Normal),
                            ]);
                        }
                    }
                }
            }
            _ => (),
        }
        Ok(vec![ActionReturn::Good])    
    }
}
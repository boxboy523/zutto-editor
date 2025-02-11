use std::io::Write;

use anyhow::Result;
use ropey::Rope;

use crate::Size;


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
pub struct BufferLine{
    pub text: String,
    pub scroll: usize,
    pub cur: usize,
    pub pos: Pos,
    pub len: usize,
    log : Vec<String>,
    log_idx: usize,
}

impl BufferLine {
    pub fn new(pos: Pos, len: usize) -> Self {
        Self {
            text: String::new(),
            scroll: 0,
            cur: 0,
            pos,
            len,
            log: Vec::new(),
            log_idx: 0,
        }
    }

    pub fn cursor_forward(&mut self) {
        if self.cur < self.text.len() {
            self.cur += 1;
        }
        if self.cur > self.scroll + self.len {
            self.scroll += 1;
        }
    }

    pub fn cursor_backward(&mut self) {
        if self.cur > 0 {
            self.cur -= 1;
        }
        if self.cur < self.scroll {
            self.scroll -= 1;
        }
    }

    pub fn insert_char(&mut self, c: char, upper: bool) {
        let c = if upper { c } else { c.to_lowercase().next().unwrap() };
        self.text.insert(self.cur, c);
        self.cursor_forward();
    }

    pub fn cursor_start(&mut self) {
        self.cur = 0;
    }

    pub fn cursor_end(&mut self) {
        self.cur = self.text.len();
    }

    pub fn delete(&mut self) {
        if self.text.len() > 0 && self.cur > 0 {
            self.text.remove(self.cur - 1);
            self.cursor_backward();
        }
    }

    pub fn delete_back(&mut self) {
        if self.text.len() > 0 && self.cur < self.text.len() {
            self.text.remove(self.cur);
        }
    }

    pub fn load_prev(&mut self) {
        if self.log_idx > 0 {
            self.log_idx -= 1;
            self.text = self.log[self.log_idx].clone();
            self.cur = self.text.len();
            self.scroll = self.cur.saturating_sub(self.len);
        }
    }

    pub fn load_next(&mut self) {
        if self.log_idx < self.log.len() - 1 {
            self.log_idx += 1;
            self.text = self.log[self.log_idx].clone();
            self.cur = self.text.len();
            self.scroll = self.cur.saturating_sub(self.len);
        }
    }

    pub fn clear(&mut self) {
        self.log.push(self.text.clone());
        self.text.clear();
        self.cur = 0;
        self.scroll = 0;
        self.log_idx = self.log.len();
    }
}

#[derive(Debug)]
pub struct Buffer {
    pub text: Rope,
    pub cursor: Cursor,
    pub camera: Camera,
    pub size: Size,
    pub pos: Pos,
    pub path: Option<String>, //None if it is a new buffer
}

impl Buffer {
    pub fn new(size: Size, pos: Pos) -> Self {
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

    pub fn from_file(size: Size, pos: Pos, path: &str) -> Result<Self> {
        let text = Rope::from_reader(std::fs::File::open(path)?)?;
        Ok(Self {
            text,
            cursor: Cursor { row: 0, col: 0 },
            camera: Camera { row: 0, col: 0 },
            size,
            pos,
            path: Some(path.to_string()), 
        })
    }

    pub fn save(&self, p: Option<String>) -> Result<()> {
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

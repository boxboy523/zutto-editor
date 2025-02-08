use anyhow::Result;
use log::debug;
use ropey::Rope;

use crate::Size;



pub struct Cursor {
    pub row: usize,
    pub col: usize,
}

pub struct Buffer {
    pub text: Rope,
    pub cursor: Cursor,
    pub scroll: usize,
    pub size: Size,
}

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
    if line_slice.char(line_slice.len_chars() - 1) == '\n' {
        return line_slice.len_chars() - 1;
    } else {
        return line_slice.len_chars();
    }
}

impl Buffer {
    pub fn new(size: Size) -> Self {
        Self {
            text: Rope::new(),
            cursor: Cursor {
                row: 0,
                col: 0,
            },
            scroll: 0,
            size,
        }
    }

    pub fn read(&mut self, path: &str) -> Result<()> {
        self.text = Rope::from_reader(std::fs::File::open(path)?)?;
        Ok(())
    }

    pub fn cursor_up(&mut self) {
        if self.cursor.row > 0 {
            self.cursor.row -= 1;
            if self.cursor.col > len_no_newline(&self.text, self.cursor.row) {
                self.cursor.col = self.text.line(self.cursor.row).len_chars() - 1;
            }
        }
        if self.cursor.row < self.scroll {
            self.scroll -= 1;
        }
    }

    pub fn cursor_down(&mut self) {
        if self.text.len_lines() > 0 && self.cursor.row < self.text.len_lines() - 1  {
            self.cursor.row += 1;
            if self.cursor.col > len_no_newline(&self.text, self.cursor.row) {
                self.cursor.col = len_no_newline(&self.text, self.cursor.row);
            }
        }
        if self.cursor.row > self.scroll + self.size.height as usize - 1 {
            self.scroll += 1;
        }
    }

    pub fn cursor_forward(&mut self) {
        if self.cursor.col < len_no_newline(&self.text, self.cursor.row) {
            self.cursor.col += 1;
        } else if self.cursor.row < self.text.len_lines() - 1 {
            self.cursor.row += 1;
            self.cursor.col = 0;
        }
    }

    pub fn cursor_backward(&mut self) {
        if self.cursor.col > 0 {
            self.cursor.col -= 1;
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
        debug!("row: {}, col: {}", self.cursor.row, self.cursor.col);
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

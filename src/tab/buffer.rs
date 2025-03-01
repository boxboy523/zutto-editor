use core::sync;
use std::{cmp::min, io::Write, path::{self, PathBuf}};

use anyhow::Result;
use async_trait::async_trait;
use crossterm::{cursor, queue, style::{Color, Print, StyledContent, Stylize}, terminal::{self, Clear}};
use log::debug;
use ropey::Rope;
use syntect::{easy::HighlightLines, highlighting::{self, Theme, ThemeSet}, parsing::{SyntaxReference, SyntaxSet}};

use crate::{actions::ActionReturn, syncol_to_crosscol, Action, KeymapState, Setting};

use super::{numlen, Cursor, Pos, Size, Tab};

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub row: u16,
    pub col: u16,
}

#[derive(Debug)]
pub struct Buffer {
    pub tab_idx: usize,
    text: Rope,
    cursor_idx: usize,
    camera: Camera,
    size: Size,
    pos: Pos,
    path: Option<PathBuf>, //None if it is a new buffer
    syntax_set: SyntaxSet,
    area_start: Option<usize>,
    setting: Setting,
    saved: bool,
    theme_set: ThemeSet,
}

fn is_hangul(c: char) -> bool {
    (0xAC00 < c as u32 && 0xD7AF > c as u32) || (0x3130 < c as u32 && 0x318E > c as u32)
}

fn get_syntex_ref<'a>(text: &Rope, path: &Option<PathBuf>, syntax_set: &'a SyntaxSet) -> &'a SyntaxReference {
    match path {
        Some(p) => {
            if let Some(extension) = p.extension() {
                syntax_set.find_syntax_by_extension(extension.to_str().unwrap())
            } else {
                for line in text.lines() {
                    if let Some(syntax) = syntax_set.find_syntax_by_first_line(line.as_str().unwrap_or("")) {
                        return syntax;
                    }
                }
                None
            }
        }
        None => {
            for line in text.lines() {
                if let Some(syntax) = syntax_set.find_syntax_by_first_line(line.as_str().unwrap_or("")) {
                    return syntax;
                }
            }
            None
        }
    }.unwrap_or_else(|| syntax_set.find_syntax_plain_text())
}

fn highlight_line<'a>(line: &'a str, syntax: &SyntaxReference, syntax_set: &SyntaxSet, theme: &Theme) -> Vec<(highlighting::Style, &'a str)> {
    let mut h = HighlightLines::new(syntax, theme);
    h.highlight_line(line, syntax_set).unwrap()
}
// Text buffer
impl Buffer {
    pub fn new(size: Size, pos: Pos, setting: Setting, tab_idx: usize) -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        Self {
            tab_idx,
            pos,
            text: Rope::new(),
            cursor_idx: 0,
            camera: Camera {
                row: 0,
                col: 0,
            },
            size,
            path: None,
            syntax_set,
            theme_set,
            area_start: None,
            setting,
            saved: false,
        }
    }

    pub fn resize(&mut self, size: Size) {
        self.size = size;
    }

    pub fn from_file(size: Size, pos: Pos, path: &PathBuf, setting: Setting, tab_idx: usize) -> Result<Self> {
        let text = Self::open(path)?;
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        Ok(Self {
            tab_idx,
            text,
            cursor_idx: 0,
            camera: Camera { row: 0, col: 0 },
            size,
            pos,
            path: Some(path.clone()), 
            syntax_set,
            theme_set,
            area_start: None,
            setting,
            saved: true,
        })
    }


    // file I/O
    // 0x01: hangul padding
    // 0x02: tab padding

    fn open(path: &PathBuf) -> Result<Rope> {
        let mut rope = Rope::from_reader(std::fs::File::open(path)?)?;
        let chars = rope.chars().collect::<Vec<_>>();
        for (i, c) in chars.iter().enumerate() {
            if *c == '\t' {
                rope.insert_char(i, '\t');
                for _ in 0..3 {
                    rope.insert_char(i + 1, '\x02');
                }
            } else if is_hangul(*c) {
                rope.insert_char(i, *c);
                rope.insert_char(i + 1, '\x01');
            } else {
                rope.insert_char(i, *c);
            }
        }
        Ok(rope)
    }

    fn save(&mut self, p: Option<&str>) -> Result<()> {
        let mut byte: Vec<u8> = Vec::new();
        for b in self.text.bytes() {
            if b != 0 || b != 1 || b != 2 {
                byte.push(b);
            }
        }
        if let Some(path) = p {
            let mut file = std::fs::File::create(path)?; 
            file.write_all(&byte)?;
        } else if let Some(path) = &self.path {
            let mut file = std::fs::File::create(path)?;
            file.write_all(&byte)?;
        } else {
            return Err(anyhow::anyhow!("No path to save, use save_as(Cmd: Ctrl+S)"));
        }
        self.saved = true;
        Ok(())
    }


    // cursor movement & row, col calculation

    fn get_row(&self) -> u16 {
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
    fn get_col(&self) -> u16 {
        let i = self.get_row_start();
        (self.cursor_idx - i) as u16
    }

    fn adj_camera(&mut self) {
        let row = self.get_row();
        let col = self.get_col();
        while row < self.camera.row {
            self.camera.row -= 1;
        }
        while row >= self.camera.row + self.size.height {
            self.camera.row += 1;
        }
        while col < self.camera.col {
            self.camera.col -= 1;
        }
        while col >= self.camera.col + self.size.width {
            self.camera.col += 1;
        }
    }

    fn cursor_up(&mut self) {
        if self.get_row() == 0 {
            return;
        }
        let col = self.get_col();
        self.cursor_idx = self.get_row_start() - 1;
        self.cursor_idx = self.get_row_start();
        self.cursor_idx += min(col as usize, self.get_row_len());
        self.adj_camera();
    }

    fn cursor_down(&mut self) {
        if self.get_row() == (self.text.len_lines() - 1) as u16 {
            return;
        }
        let col = self.get_col();
        self.cursor_idx = self.get_row_end() + 1;
        self.cursor_idx += min(col as usize, self.get_row_len());
        self.adj_camera();
    }

    fn cursor_forward(&mut self) {
        if self.cursor_idx < self.text.len_chars() {
            self.cursor_idx += 1;
        }
        self.adj_camera();
    }

    fn cursor_forward_action(&mut self) {
        self.cursor_forward();
        let chars = self.text.chars().collect::<Vec<_>>();
        if self.cursor_idx > 0 {
            if is_hangul(chars[self.cursor_idx - 1]) {
                self.cursor_forward();
            } else if chars[self.cursor_idx - 1] == '\t' {
                self.cursor_forward();
                while self.cursor_idx < self.text.len_chars() && chars[self.cursor_idx] == '\x02' {
                    self.cursor_forward();
                }
            } else if chars[self.cursor_idx - 1] == ' ' {
                if (self.get_col() as usize - 1) % self.setting.tab_size == 0 {
                    while self.cursor_idx < self.text.len_chars() && chars[self.cursor_idx] == ' ' && self.get_col() as usize % self.setting.tab_size != 0 {
                        self.cursor_forward();
                    }
                }
            }
        }
    }

    fn cursor_backward(&mut self) {
        if self.cursor_idx > 0 {
            self.cursor_idx -= 1;
        }
        self.adj_camera();
    }

    fn cursor_backward_action(&mut self) {
        self.cursor_backward();
        let chars = self.text.chars().collect::<Vec<_>>();
        if self.cursor_idx > 0{ 
            if is_hangul(chars[self.cursor_idx - 1]) {
                self.cursor_backward();
            } else if chars[self.cursor_idx - 1] == '\x02' {
                while self.cursor_idx > 0 && chars[self.cursor_idx - 1] == '\x02' {
                    self.cursor_backward();
                }
                self.cursor_backward();
            } else if chars[self.cursor_idx - 1] == ' ' {
                if (self.get_col() as usize + 1) % self.setting.tab_size == 0 {
                    while self.cursor_idx > 0 && chars[self.cursor_idx - 1] == ' ' && self.get_col() as usize % self.setting.tab_size != 0 {
                        self.cursor_backward();
                    }
                }
            }
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
        self.adj_camera();
    }

    fn cursor_end(&mut self) {
        let i = self.cursor_idx;
        for j in i..self.text.len_chars() {
            if self.text.char(j) == '\n' {
                self.cursor_idx = j;
                return;
            }
        }
        self.adj_camera();
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
        self.adj_camera();
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
        self.adj_camera();
    }

    // text manipulation

    fn insert_char(&mut self, c: char, upper: bool) {
        let c = if upper { c } else { c.to_lowercase().next().unwrap() };
        self.text.insert_char(self.cursor_idx, c);
        self.cursor_forward();
        if is_hangul(c) {
            self.text.insert_char(self.cursor_idx, '\x01');
            self.cursor_forward();
        }
        self.saved = false;
    }

    fn insert_str(&mut self, s: &str) {
        self.text.insert(self.cursor_idx, s);
        self.cursor_forward();
        self.saved = false;
    }

    fn insert_newline(&mut self) {
        self.insert_char('\n', true);
        self.cursor_forward();
        self.saved = false;
    }

    fn insert_tab(&mut self) {
        let tab_size = self.setting.tab_size - self.get_col() as usize % self.setting.tab_size;
        match self.setting.tab_type {
            crate::TabType::Space => {
                for _ in 0..tab_size {
                    self.text.insert_char(self.cursor_idx, ' ');
                    self.cursor_forward();
                }
            }
            crate::TabType::Tab => {
                self.text.insert_char(self.cursor_idx, '\t');
                self.cursor_forward();
                for _ in 0..tab_size - 1 {
                    self.text.insert_char(self.cursor_idx, '\x02');
                    self.cursor_forward();
                }
            }
        }
        self.saved = false;
    }

    fn insert_newline_above(&mut self) {
        let idx = self.get_row_start();
        self.text.insert_char(idx, '\n');
        self.saved = false;
    }

    fn insert_newline_below(&mut self) {
        let idx = self.get_row_end();
        self.text.insert_char(idx, '\n');
        self.saved = false;
    }

    fn delete(&mut self) {
        if self.text.len_chars() > 0 && self.cursor_idx > 0 {
            self.text.remove(self.cursor_idx - 1..self.cursor_idx);
            self.cursor_backward();
        }
    }

    fn delete_action(&mut self) {
        let chars = self.text.chars().collect::<Vec<_>>();
        if self.cursor_idx > 0 {
            if chars[self.cursor_idx - 1] == ' ' && self.get_col() as usize % self.setting.tab_size == 0 {
                self.delete();
                while self.cursor_idx > 0 && chars[self.cursor_idx - 1] == ' ' && self.get_col() as usize % self.setting.tab_size != 0 {
                    self.text.remove(self.cursor_idx - 1..self.cursor_idx);
                    self.cursor_backward();
                }
            }
            else if chars[self.cursor_idx - 1] == '\x01' && self.cursor_idx > 1 && is_hangul(chars[self.cursor_idx - 2]) {
                self.text.remove(self.cursor_idx - 2..self.cursor_idx);
                self.cursor_backward();
                self.cursor_backward();
            } else if chars[self.cursor_idx - 1] == '\x02' {
                while chars[self.cursor_idx - 1] == '\x02' {
                    self.delete();
                }
                self.delete();
            } else {
                self.delete();
            }
        }
        self.saved = false;
    }

    fn delete_back(&mut self) {
        if self.text.len_chars() > 0 && self.cursor_idx < self.text.len_chars() {
            self.text.remove(self.cursor_idx..self.cursor_idx + 1);
        }
        self.saved = false;
    }

    // visualization

    fn visualize(&self, line: usize, theme: &Theme, numpad: usize) -> Vec<StyledContent<String>> {
        let bg = syncol_to_crosscol(theme.settings.background.unwrap());
        let line_slice = match self.text.get_line(line) {
            Some(l) => l,
            None => return vec![" ".repeat(self.size.width as usize).on(bg)],
        };

        let mut s = line_slice.to_string();
        
        if line_slice.len_chars() == 0 {
            return vec![" ".repeat(self.size.width as usize - numpad - 1).on(bg)];
        }
        if line_slice.char(line_slice.len_chars() - 1) == '\n' {
            s.pop().unwrap();
        }
        if self.camera.col as usize > s.chars().count() {
            return vec![" ".repeat(self.size.width as usize - numpad - 1).on(bg)];
        }
        let syntax = get_syntex_ref(&self.text, &self.path, &self.syntax_set);
        let h = highlight_line(&s, syntax, &self.syntax_set, theme);
        let mut styled = Vec::new();
        let mut len = 0;
        for (style, s) in h {
            let fg = syncol_to_crosscol(style.foreground);
            let bg = syncol_to_crosscol(style.background);
            len += s.chars().count();
            let s = s.to_string().on(bg).with(fg);
            styled.push(s);
        }
        if len + numpad < self.size.width as usize {
            styled.push(" ".repeat(self.size.width as usize - len - numpad - 1).on(bg));
        }
        styled
    }

    pub fn render<W>(&self, write: &mut W) -> Result<()>
    where
        W: Write,
    {
        let camera = self.camera;
        let line_len = self.text.len_lines();
        let line_num_padding = if self.setting.line_numbers {
            numlen(line_len) + 1
        } else {
            0
        };
        for i in 0..self.size.height as usize {
            let line = self.visualize(i + camera.row as usize, &self.theme_set.themes["base16-ocean.dark"], line_num_padding);
            queue!(
                write,
                cursor::MoveTo(self.pos.col, i as u16 + self.pos.row),
                //Clear(terminal::ClearType::UntilNewLine),
            )?;
            if self.setting.line_numbers {
                let line_num = format!("{:width$} ", i + 1 + camera.row as usize, width = line_num_padding);
                if i + camera.row as usize == self.get_row() as usize {
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
            for s in line {
                queue!(write, Print(s))?;
            }
        }
        Ok(())
    }

    fn get_pos(&self) -> Pos {
        self.pos
    }

    pub fn name(&self) -> String {
        match &self.path {
            Some(p) => p.file_name().unwrap().to_string_lossy().to_string(),
            None => "Untitled".to_string(),
        }
    }

    fn get_size(&self) -> Size {
        self.size
    }

    pub fn get_cursor(&self) -> Option<Cursor> {
        let line_num_padding = if self.setting.line_numbers {
            numlen(self.text.len_lines()) + 2
        } else {
            0
        };
        let mut cursor = Cursor {
            row: self.get_row(),
            col: self.get_col(),
        };
        cursor.col += line_num_padding as u16;
        cursor.col -= self.camera.col;
        cursor.col += self.pos.col;
        cursor.row -= self.camera.row;
        cursor.row += self.pos.row;
        Some(cursor)
    }
    pub async fn process_action(&mut self, action: &Action) -> Result<Vec<ActionReturn>> {
        let action_name = &action.name;
        let mut action_args = action.args.clone();
        match action_name.as_str() {
            "CursorUp" => { self.cursor_up(); }
            "CursorDown" => { self.cursor_down(); }
            "CursorForward" => { self.cursor_forward_action(); }
            "CursorBackward" => { self.cursor_backward_action(); }
            "CursorForwardWord" => { self.cursor_forward_word(); }
            "CursorBackwardWord" => { self.cursor_backward_word(); }
            "CursorStart" => { self.cursor_start(); }
            "CursorEnd" => { self.cursor_end(); }
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
            "InsertNewline" => { self.insert_newline(); }
            "InsertNewlineAbove" => { self.insert_newline_above(); }
            "InsertNewlineBelow" => { self.insert_newline_below(); }
            "InsertSpace" => { self.insert_str(" "); }
            "InsertComma" => { self.insert_str(","); }
            "InsertTab" => { self.insert_tab(); }
            "Delete" => { self.delete_action(); }
            "DeleteBack" => { self.delete_back(); }
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
                            self.path = Some(PathBuf::from(action_args[0].as_ref().unwrap()));
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
        Ok(vec![])    
    }
}
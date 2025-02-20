use std::{fmt::Debug, io::Write};

use anyhow::Result;

use crate::{actions::ActionReturn, Action};

pub mod buffer;
pub mod directory;
pub mod terminal;

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


#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: u16,
    pub height: u16,
}

pub trait Tab: Debug {
    fn render(&self, write: &mut Box<dyn Write>) -> Result<()>;
    fn get_pos(&self) -> Pos;
    fn get_name(&self) -> String;
    fn get_size(&self) -> Size;
    fn get_cursor(&self) -> Option<Cursor>;
    fn process_action(&mut self, action: &Action) -> Result<Vec<ActionReturn>>;
}

pub fn numlen (mut num: usize) -> usize {
    let mut len = 0;
    while num > 0 {
        num /= 10;
        len += 1;
    }
    len
}
use std::{fmt::Debug, io::Write};

use anyhow::Result;
use async_trait::async_trait;
use syntect::highlighting::Theme;

use crate::{actions::ActionReturn, Action};

pub mod buffer;
pub mod directory;
pub mod shell;

#[derive(Debug, Clone, Copy)]
pub struct Pos {
    pub row: u16,
    pub col: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct Cursor {
    pub row: u16,
    pub col: u16,
}


#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: u16,
    pub height: u16,
}

#[derive(Debug)]
pub enum Tab{
    Buffer(buffer::Buffer),
    Directory(directory::Directory),
    Shell(shell::Shell),
}

pub fn numlen (mut num: usize) -> usize {
    let mut len = 0;
    while num > 0 {
        num /= 10;
        len += 1;
    }
    len
}
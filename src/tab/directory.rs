use std::{io::Write, iter, path::PathBuf};

use anyhow::Result;
use async_trait::async_trait;
use crossterm::{cursor, execute, queue, style::{style, Print, Stylize}, terminal::{Clear, ClearType}};
use log::debug;
use syntect::highlighting::Theme;

use crate::actions::ActionReturn;

use super::{Cursor, Pos, Size, Tab};

#[derive(Debug)]
pub struct Directory {
    pub tab_idx: usize,
    path: PathBuf,
    files: Vec<PathBuf>,
    scroll: usize,
    selected: usize,
    pos : Pos,
    size: Size,
}

impl Directory {
    pub fn new(path: PathBuf, pos: Pos, size: Size, tab_idx:usize) -> Result<Self> {
        let files = std::fs::read_dir(&path).unwrap()
            .map(|res| res.map(|e| e.path()))
            .collect::<std::result::Result<Vec<_>, std::io::Error>>()?;

        Ok(Self {
            tab_idx,
            path,
            files,
            scroll: 0,
            selected: 0,
            pos,
            size,
        })
    }

    pub fn render<W>(&self, write: &mut W) -> Result<()> 
        where W: Write
    {
        let file_names = self.files.iter().skip(self.scroll)
            .map(|f| format!("{} {}", get_file_icon(f), f.file_name().unwrap().to_str().unwrap())).chain(iter::once("..".to_string()));
        for (i, file) in file_names.enumerate() {
            if i >= self.size.height as usize {
                break;
            }
            if i == self.selected {
                queue!(
                    write,
                    cursor::MoveTo(self.pos.col, self.pos.row + i as u16),
                    Print(style("> ").reverse()),
                    Print(file.reverse()),
                )?;
            } else {
                queue!(
                    write,
                    cursor::MoveTo(self.pos.col, self.pos.row + i as u16),
                    Print(file),
                )?;
            }
            queue!(
                write,
                Clear(ClearType::UntilNewLine),
            )?;
        }
        Ok(())
    }
    pub fn get_cursor(&self) -> Option<Cursor> {
        None
    }
    pub fn name(&self) -> String {
        self.path.to_str().unwrap().to_string()
    }
    fn get_pos(&self) -> Pos {
        self.pos
    }
    fn get_size(&self) -> Size {
        self.size
    }
    pub async fn process_action(&mut self, action: &crate::Action) -> anyhow::Result<Vec<ActionReturn>> {
        let select_len = self.files.len() + 1;
        match action.name.as_str() {
            "CursorUp" => {
                self.selected = (self.selected + select_len - 1) % select_len;
            }
            "CursorDown" => {
                self.selected = (self.selected + 1) % select_len;
            }
            "InsertNewline" => {
                let mut path;
                if self.selected == select_len - 1 {
                    path = self.path.parent().unwrap().to_path_buf();
                    if path.to_str().unwrap() == "" {
                        path = self.path.canonicalize().unwrap().parent().unwrap().to_path_buf();
                    }
                }
                else {
                    path = self.files[self.selected].clone();
                }
                if path.is_dir() {
                    return Ok(vec![ActionReturn::NewDir(path), ActionReturn::CloseTab(self.tab_idx)]);
                } else if path.is_file() {
                    return Ok(vec![ActionReturn::NewBuffer(Some(path))]);
                }
            }
            _ => {}
        }
        Ok(vec![])
    }
}

fn get_file_icon(file: &PathBuf) -> String {
    if file.is_dir() {
        return "".to_string();
    }
    let ext = match file.extension() {
        Some(e) => e.to_str().unwrap(),
        None => "",
    };
    match ext {
        "rs" => "",
        "toml" => "",
        "json" => "",
        "yaml" => "",
        "yml" => "",
        "md" => "",
        "txt" => "",
        "sh" => "",
        "zsh" => "",
        "bash" => "",
        "py" => "",
        "c" => "",
        "cpp" => "",
        "h" => "",
        "hpp" => "",
        "go" => "",
        "java" => "",
        "js" => "",
        "ts" => "",
        "html" => "",
        "css" => "",
        "scss" => "",
        "png" => "󰋩",
        "jpg" => "󰋩",
        "jpeg" => "󰋩",
        "gif" => "󰋩",
        "svg" => "󰋩",
        "mp4" => "󰕧",
        "avi" => "󰕧",
        "mkv" => "󰕧",
        "mov" => "󰕧",
        "mp3" => "",
        "flac" => "",
        "wav" => "",
        "ogg" => "",
        "zip" => "",
        "tar" => "",
        "gz" => "",
        "7z" => "",
        "rar" => "",
        "pdf" => "",
        _ => "",
    }.to_string()
}

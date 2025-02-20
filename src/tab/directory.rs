use std::path::PathBuf;

use anyhow::Result;
use crossterm::{cursor, execute, style::Print};

use crate::actions::ActionReturn;

use super::{Cursor, Pos, Size, Tab};

#[derive(Debug)]
pub struct Directory {
    pub path: PathBuf,
    pub files: Vec<PathBuf>,
    pub scroll: usize,
    pub pos : Pos,
    pub size: Size,
}

impl Directory {
    pub fn new(path: PathBuf, pos: Pos, size: Size) -> Self {
        let files = std::fs::read_dir(&path).unwrap()
            .map(|res| res.map(|e| e.path()))
            .collect::<std::result::Result<Vec<_>, std::io::Error>>().unwrap();
        Self {
            path,
            files,
            scroll: 0,
            pos,
            size,
        }
    }
}

impl Tab for Directory {
    fn render(&self, write: &mut Box<dyn std::io::Write>) -> Result<()> {
        for (i, file) in self.files.iter().enumerate() {
            let name = file.file_name().unwrap().to_str().unwrap();
            let icon = get_file_icon(file.clone());
            let print = format!("{} {}", icon, name);
            execute!(
                write,
                cursor::MoveTo(0, i as u16),
                Print(print),
            )?;
        }
        Ok(())
    }
    fn get_cursor(&self) -> Option<Cursor> {
        None
    }
    fn get_name(&self) -> String {
        self.path.to_str().unwrap().to_string()
    }
    fn get_pos(&self) -> Pos {
        self.pos
    }
    fn get_size(&self) -> Size {
        self.size
    }
    fn process_action(&mut self, action: &crate::Action) -> anyhow::Result<Vec<ActionReturn>> {
        Ok(vec![ActionReturn::Good])
    }
}

fn get_file_icon(file: PathBuf) -> String {
    if file.is_dir() {
        return "".to_string();
    }
    let ext = file.extension().unwrap().to_str().unwrap();
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

use std::{fs, path::{Path, PathBuf}};

use crate::{Action, KeymapState};
use anyhow::{Error, Result};
pub enum ActionReturn {
    Continue,
    Stop,
    Err(Error),
    Excute(Action),
    ExcuteLine(String),
    NewBuffer(Option<PathBuf>),
    NewDir(PathBuf),
    NewShell,
    State(KeymapState),
    Notice(String),
    ChangeTab(isize),
    CloseTab(usize),
} 

pub fn normal_mode(_: &Action) -> Result<Vec<ActionReturn>> {
    Ok(vec![ActionReturn::State(KeymapState::Normal)])
}

pub fn cmd_mode(_: &Action) -> Result<Vec<ActionReturn>> {
    Ok(vec![ActionReturn::State(KeymapState::Cmd)])
}

pub fn quit(_: &Action) -> Result<Vec<ActionReturn>> {
    Ok(vec![ActionReturn::Stop])
}

pub fn find_mode(_: &Action) -> Result<Vec<ActionReturn>> {
    Ok(vec![ActionReturn::State(KeymapState::Find)])
}

pub fn line_mode(_: &Action) -> Result<Vec<ActionReturn>> {
    Ok(vec![ActionReturn::State(KeymapState::LineInsert)])
}

pub fn next_tab(_: &Action) -> Result<Vec<ActionReturn>> {
    Ok(vec![ActionReturn::ChangeTab(1)])
}

pub fn prev_tab(_: &Action) -> Result<Vec<ActionReturn>> {
    Ok(vec![ActionReturn::ChangeTab(-1)])
}

pub fn open(action: &Action) -> Result<Vec<ActionReturn>> {
    if action.args[0].is_none() {
        return Ok(vec![
            ActionReturn::State(KeymapState::LineInsert),
            ActionReturn::Notice("Enter file name: ".to_string()),
            ActionReturn::ExcuteLine("Open($line)".to_string()),
        ]);
    } else {
        let path = Path::new(action.args[0].as_ref().unwrap());
        if path.is_file() {
            return Ok(vec![
                ActionReturn::Notice(format!("Opened {}", path.display())),
                ActionReturn::NewBuffer(Some(path.to_path_buf())),
                ActionReturn::State(KeymapState::Normal),
            ]);
        } else if path.is_dir() {
            return Ok(vec![
                ActionReturn::Notice(format!("Opened {}", path.display())),
                ActionReturn::NewDir(path.to_path_buf()),
                ActionReturn::State(KeymapState::Normal),
            ]);
        } else {
            return Ok(vec![
                ActionReturn::Notice(format!("{} is not a file", path.display())),
                ActionReturn::State(KeymapState::LineInsert),
            ]);
        }
    }
}

pub fn close_tab(action: &Action) -> Result<Vec<ActionReturn>> {
    let tab_idx = action.args[0].as_ref().unwrap().parse::<usize>().unwrap();
    Ok(vec![ActionReturn::CloseTab(tab_idx)])
}

pub fn new_shell(_: &Action) -> Result<Vec<ActionReturn>> {
    Ok(vec![ActionReturn::NewShell])
}
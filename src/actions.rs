use crate::{Action, KeymapState};
use anyhow::Error;
pub enum ActionReturn {
    Good,
    Continue,
    Stop,
    Err(Error),
    Excute(Action),
    ExcuteLine(String),
    NewBuffer(Option<String>),
    State(KeymapState),
    Notice(String),
    ChangeTab(isize),
} 

pub fn normal_mode(_: &Action) -> Vec<ActionReturn> {
    vec![ActionReturn::State(KeymapState::Normal)]
}

pub fn cmd_mode(_: &Action) -> Vec<ActionReturn> {
    vec![ActionReturn::State(KeymapState::Cmd)]
}

pub fn quit(_: &Action) -> Vec<ActionReturn> {
    vec![ActionReturn::Stop]
}

pub fn find_mode(_: &Action) -> Vec<ActionReturn> {
    vec![ActionReturn::State(KeymapState::Find)]
}

pub fn line_mode(_: &Action) -> Vec<ActionReturn> {
    vec![ActionReturn::State(KeymapState::LineInsert)]
}

pub fn next_tab(_: &Action) -> Vec<ActionReturn> {
    vec![ActionReturn::ChangeTab(1)]
}

pub fn prev_tab(_: &Action) -> Vec<ActionReturn> {
    vec![ActionReturn::ChangeTab(-1)]
}
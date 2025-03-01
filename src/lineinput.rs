use anyhow::Result;

use crate::{actions::ActionReturn, parse_action, Action};

#[derive(Debug)]
pub struct LineInput{
    pub text: String,
    pub scroll: usize,
    pub cur: usize,
    pub len: usize,
    pub action: Option<String>,
    pub notice: String,
    log : Vec<String>,
    log_idx: usize,
}

impl LineInput {
    pub fn new(len: usize) -> Self {
        Self {
            text: String::new(),
            scroll: 0,
            cur: 0,
            len,
            action: None,
            notice: String::new(),
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
        self.action = None;
        self.cur = 0;
        self.scroll = 0;
        self.log_idx = self.log.len();
    }

    pub fn process_action(&mut self, action: &Action, idx: usize) -> Result<Vec<ActionReturn>> {
        let action_name = &action.name;
        let mut action_args = action.args.clone();
        match action_name.as_str() {
            "LineInsert" => {
                let c = action_args[0].as_mut().unwrap().chars().next().unwrap();
                self.insert_char(c, false);
            }
            "LineInsertUpper" => {
                let c = action_args[0].as_mut().unwrap().chars().next().unwrap();
                self.insert_char(c, true);
            }
            "LineInsertSpace" => {
                self.insert_char(' ', false);
            }
            "LineCursorForward" => {
                self.cursor_forward();
            }
            "LineCursorBackward" => {
                self.cursor_backward();
            }
            "LineStart" => {
                self.cursor_start();
            }
            "LineEnd" => {
                self.cursor_end();
            }
            "LineDelete" => {
                self.delete();
            }
            "LineDeleteBackward" => {
                self.delete_back();
            }
            "LineExecute" => {
                let action = self.action.clone();
                match action {
                    Some(a) => {
                        self.action = None;
                        let action = parse_action(&a, &self.text, idx);
                        self.clear();
                        match action{
                            Ok(a) => return Ok(vec![ActionReturn::Excute(a)]),
                            Err(e) => return Ok(vec![ActionReturn::Err(e)]),
                        };
                    }
                    None => {
                        let action = parse_action(&self.text, &self.text, idx);
                        self.clear();
                        match action {
                            Ok(a) => return Ok(vec![ActionReturn::Excute(a)]),
                            Err(e) => return Ok(vec![ActionReturn::Err(e)]),
                        };
                    }
                }
            }
            _ => (),
        }
        Ok(vec![])
    }
}
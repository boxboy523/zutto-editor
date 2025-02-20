use std::{collections::{BTreeSet, HashMap}, hash::Hash, str::FromStr};

use anyhow::{Ok, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use log::debug;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use strum_macros::{EnumString, IntoStaticStr};

use crate::KeymapState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, IntoStaticStr, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Key {
    CharAny,
    Ctrl,
    Alt,
    Shift,
    Char(char),
    F(u8),
    CapsLock,
    Tab,
    Enter,
    Backspace,
    Space,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    Esc,
    NumLock,
    ScrollLock,
    Pause,
    PrintScreen,
    Break,
    Escape,
    BackTab,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
struct Command {
    key: BTreeSet<BTreeSet<Key>>,
}

fn keycode_to_key(k: KeyCode) -> Key {
    let s = match k {
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char(c) => format!("{}", c.to_uppercase()),
        KeyCode::F(n) => format!("F{}", n),
        _ => format!("{:?}", k),
    };
    if s.len() == 1 {
        Key::Char(s.chars().next().unwrap())
    } else if s.len() < 4 && s.starts_with('F') {
        Key::F(s[1..].parse().unwrap())
    } else {
        Key::from_str(s.as_str()).unwrap()
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Keymap {
    keymap: HashMap<String, Command>,
    #[serde(skip)]
    keymap_reversed: HashMap<Command, String>,
}

impl Keymap {
    pub fn get_action(&self, key: &BTreeSet<Key>) -> Option<String> {
        let is_char = |k: &Key| {
            match k {
                Key::Char(_) => true,
                _ => false,
            }
        };
        for (command, action) in &self.keymap_reversed {
            for k in &command.key {
                if k == key {
                    return Some(action.clone());
                } else if k.contains(&Key::CharAny) && key.iter().any(is_char) {
                    let mut com = k.clone();
                    com.remove(&Key::CharAny);
                    let char_code = key.iter().find(|x| is_char(x)).unwrap();
                    let key = key.iter().filter(|x| !is_char(x)).cloned().collect::<BTreeSet<_>>();
                    if key == com {
                        if let Key::Char(c) = char_code {
                            let idx = action.find("$char");
                            match idx {
                                Some(idx) => {
                                    let mut rtn = action.clone();
                                    rtn.replace_range(idx..idx+5, &c.to_string());
                                    return Some(rtn);
                                }
                                None => unreachable!("It contains Key::CharAny but the action not contains char field."),
                            }
                        }
                    }
                }
            }
        }
        None
    }
    pub fn read(event: KeyEvent) -> Option<BTreeSet<Key>> {
        let mut rtn = BTreeSet::new();
        debug!("{:?}", event);
        match event {
            KeyEvent {
                modifiers,
                code,
                ..
            } => {
                if modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT) {
                    rtn.insert(Key::Ctrl);
                    rtn.insert(Key::Shift);
                    rtn.insert(keycode_to_key(code));
                }
                else if modifiers == (KeyModifiers::ALT | KeyModifiers::SHIFT) {
                    rtn.insert(Key::Alt);
                    rtn.insert(Key::Shift);
                    rtn.insert(keycode_to_key(code));
                }
                else if modifiers == (KeyModifiers::CONTROL | KeyModifiers::ALT) {
                    rtn.insert(Key::Ctrl);
                    rtn.insert(Key::Alt);
                    rtn.insert(keycode_to_key(code));
                }
                else if modifiers == (KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT) {
                    rtn.insert(Key::Ctrl);
                    rtn.insert(Key::Alt);
                    rtn.insert(Key::Shift);
                    rtn.insert(keycode_to_key(code));
                }
                else if modifiers == KeyModifiers::CONTROL {
                    rtn.insert(Key::Ctrl);
                    rtn.insert(keycode_to_key(code));
                }
                else if modifiers == KeyModifiers::ALT {
                    rtn.insert(Key::Alt);
                    rtn.insert(keycode_to_key(code));
                }
                else if modifiers == KeyModifiers::SHIFT {
                    rtn.insert(Key::Shift);
                    rtn.insert(keycode_to_key(code));
                }
                else {
                    rtn.insert(keycode_to_key(code));
                }
            }
        };
        if rtn.is_empty() {
            return None;
        }
        Some(rtn)
    }
}


pub fn open_keymaps(path: &str) -> Result<HashMap<KeymapState, Keymap>> {
    let file = std::fs::File::open(path)?;
    let json: Value = serde_json::from_reader(file)?;
    let mut rtn: HashMap<KeymapState, Keymap> = serde_json::from_value(json)?;
    for (_, keymap) in &mut rtn {
        for (action, command) in &mut keymap.keymap {
            keymap.keymap_reversed.insert(command.clone(), action.clone());
        }
    }
    Ok(rtn)
}

#[cfg(test)]
mod test{
    use super::*;

    #[test]
    fn deserialize() {
        let mut keymap = Keymap {
            keymap: HashMap::new(),
            keymap_reversed: HashMap::new(),
        };
        let mut command = Command {
            key: BTreeSet::new(),
        };
        command.key.insert(BTreeSet::from_iter(vec![Key::Ctrl, Key::Char('c')]));
        keymap.keymap.insert(String::from("Quit"), command.clone());
        keymap.keymap_reversed.insert(command, String::from("Quit"));
        println!("{}", serde_json::to_string(&keymap).unwrap());
        assert_eq!( serde_json::to_string(&keymap).unwrap(), "{\"Quit\":[[\"Ctrl\",{\"Char\":\"c\"}]]}");
    }

    #[test]
    fn serialize() {
        let keymap = "{\"Quit\":[[\"Ctrl\",{\"Char\":\"c\"}]]}";
        let keymap: Keymap = serde_json::from_str(keymap).unwrap();
        let mut command = Command {
            key: BTreeSet::new(),
        };
        command.key.insert(BTreeSet::from_iter(vec![Key::Ctrl, Key::Char('c')]));
        assert_eq!(keymap.keymap.get("Quit").unwrap(), &command);
    }

    #[test]
    fn setting() {
        let keymanager = open_keymaps("settings/keymap.json").unwrap();
        println!("{:?}", keymanager.get(&KeymapState::Normal));
    }

    #[test]
    fn keymap() {
        let keymanager = open_keymaps("settings/keymap.json").unwrap();
        let keymap = keymanager.get(&KeymapState::Normal).unwrap();
        let action = keymap.get_action(&BTreeSet::from_iter(vec![Key::Char('c')])).unwrap();
        assert_eq!(action, "Insert(C)");
    }
}
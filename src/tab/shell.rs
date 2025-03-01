use std::{process::Stdio, sync::Arc, thread::spawn};

use anyhow::Result;
use async_trait::async_trait;
use crossterm::{cursor, queue, style::Print, terminal::{Clear, ClearType}};
use log::{debug, error};
use ropey::Rope;
use syntect::highlighting::Theme;
use tokio::{io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader}, process::{Child, ChildStdin, Command}, sync::{mpsc::{Receiver, Sender}, Mutex}};

use super::{Pos, Size, Tab};

#[derive(Debug)]
pub struct Shell {
    pub tab_idx: usize,
    log: Arc<Mutex<Rope>>,
    pub size: Size,
    stdout_rx: Arc<Mutex<Receiver<u8>>>,
    stderr_rx: Arc<Mutex<Receiver<u8>>>,
    stdin: ChildStdin,
    line_input: String,
    cursor: usize,
    pub pos: Pos,
    out_buf: Arc<Mutex<[u8; 4]>>,
    err_buf: Arc<Mutex<[u8; 4]>>,
}


impl Shell {
    pub fn new(pos: Pos, size: Size, tab_idx: usize) -> Self {
        let shell_path = "sh";
        let mut shell = Command::new(shell_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = shell.stdout.take().unwrap();
        let stderr = shell.stderr.take().unwrap();
        let stdin = shell.stdin.take().unwrap();
        let (stdout_tx, stdout_rx) = tokio::sync::mpsc::channel(10000);
        let (stderr_tx, stderr_rx) = tokio::sync::mpsc::channel(10000);
        Self::spawn_reader(stdout_tx, stderr_tx, BufReader::new(stdout), BufReader::new(stderr));
        tokio::spawn(async move {
            let status = shell.wait().await.unwrap();
            debug!("Shell exited with: {}", status);
        });

        let log = Arc::new(Mutex::new(Rope::new()));
        let out_buf = Arc::new(Mutex::new([0; 4]));
        let err_buf = Arc::new(Mutex::new([0; 4]));
        let stdout_rx = Arc::new(Mutex::new(stdout_rx));
        let stderr_rx = Arc::new(Mutex::new(stderr_rx));

        let log_clone = Arc::clone(&log);
        let out_buf_clone = Arc::clone(&out_buf);
        let err_buf_clone = Arc::clone(&err_buf);
        let stdout_rx_clone = Arc::clone(&stdout_rx);
        let stderr_rx_clone = Arc::clone(&stderr_rx);

        tokio::spawn(
            async move {
            loop {
                Self::read_stdout(stdout_rx_clone.clone(), out_buf_clone.clone(), log_clone.clone()).await;
                Self::read_stderr(stderr_rx_clone.clone(), err_buf_clone.clone(), log_clone.clone()).await;
            }
        });

        Self {
            tab_idx,
            log,
            stdout_rx,
            stderr_rx,
            stdin,
            size,
            pos,
            out_buf,
            err_buf,
            line_input: String::new(),
            cursor: 0,
        }
    }
    fn spawn_reader(stdout_tx: Sender<u8>, stderr_tx: Sender<u8>, mut reader: BufReader<tokio::process::ChildStdout>, mut err_reader: BufReader<tokio::process::ChildStderr>) {
        tokio::spawn(async move {
            let mut buf = [0];
            loop {
                match reader.read(&mut buf).await {
                    Ok(n) => {
                        if n != 0 {
                            stdout_tx.send(buf[0]).await.unwrap();
                        }
                    }
                    Err(e) => {
                        error!("Error reading stdout: {}", e);
                        break;
                    }
                }
            }
        });
        tokio::spawn(async move {
            let mut buf = [0];
            loop {
                match err_reader.read(&mut buf).await {
                    Ok(n) => {
                        if n != 0 {
                            stderr_tx.send(buf[0]).await.unwrap();
                        }
                    }
                    Err(e) => {
                        error!("Error reading stderr: {}", e);
                        break;
                    }
                }
            }
        });
    }

    async fn read_stdout(stdout_rx_mut: Arc<Mutex<Receiver<u8>>>, buf_mut: Arc<Mutex<[u8; 4]>>, rope: Arc<Mutex<Rope>>){
        let mut stdout_rx = stdout_rx_mut.lock().await;
        let mut buf = buf_mut.lock().await;
        while let Ok(line) = stdout_rx.try_recv() {
            let mut len = 0;
            for (i, c) in buf.iter_mut().enumerate() {
                if c == &0 {
                    *c = line;
                    len = i + 1;
                    break;
                }
            }
            if let Ok(s) = String::from_utf8(buf[..len].to_vec()) {
                *buf = [0; 4];
                let mut rope = rope.lock().await;
                rope.append(s.into());
            }
        }
    }

    async fn read_stderr(stderr_rx_mut: Arc<Mutex<Receiver<u8>>>, buf_mut: Arc<Mutex<[u8; 4]>>, rope: Arc<Mutex<Rope>>){
        let mut stderr_rx = stderr_rx_mut.lock().await;
        let mut buf = buf_mut.lock().await;
        while let Ok(line) = stderr_rx.try_recv() {
            let mut len = 0;
            for (i , c) in buf.iter_mut().enumerate() {
                if c == &0 {
                    *c = line;
                    len = i + 1;
                    break;
                }
            }
            if let Ok(s) = String::from_utf8(buf[..len].to_vec()) {
                *buf = [0; 4];
                let mut rope = rope.lock().await;
                rope.append(s.into());
            }
        }
    }

    fn insert_char(&mut self, c: char, upper: bool) {
        let c = if upper { c } else { c.to_lowercase().next().unwrap() };
        debug!("Inserting char: {}", c);
        self.line_input.insert(self.cursor, c);
        self.cursor_forward();
    }

    fn cursor_forward(&mut self) {
        if self.cursor < self.line_input.len() {
            self.cursor += 1;
        }
    }

    fn cursor_backward(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub async fn render<W>(&self, write: &mut W) -> Result<()> 
    where W: std::io::Write
    {
        queue!(
            write,
            cursor::MoveTo(self.pos.col, self.pos.row + self.size.height as u16 - 1),
            Clear(ClearType::UntilNewLine),
            Print("> "),
            Print(self.line_input.as_str())
        )?;
        let log= self.log.lock().await;
        for (i, line) in log.lines().enumerate() {
            queue!(
                write,
                cursor::MoveTo(self.pos.col, self.pos.row + i as u16),
                Print(line)
            )?;
        }
        Ok(())
    }
    
    pub fn get_cursor(&self) -> Option<super::Cursor> {
        Some(super::Cursor {
            row: self.pos.row + self.size.height - 1,
            col: self.pos.col + 2 + self.cursor as u16,
        })
    }
    
    pub fn name(&self) -> String {
        "Shell".to_string()
    }
    pub async fn process_action(&mut self, action: &crate::Action) -> anyhow::Result<Vec<super::ActionReturn>> {
        match action.name.as_str() {
            "Insert" => {
                self.insert_char(action.args[0].as_ref().unwrap().chars().next().unwrap(), false);
            }
            "InsertUpper" => {
                self.insert_char(action.args[0].as_ref().unwrap().chars().next().unwrap(), true);
            }
            "InsertSpace" => {
                self.insert_char(' ', false);
            }
            "Delete" => {
                self.line_input.pop();
                self.cursor_backward();
            }
            "CursorForward" => {
                self.cursor_forward();
            }
            "CursorBackward" => {
                self.cursor_backward();
            }
            "InsertNewline" => {
                self.stdin.write(self.line_input.as_bytes()).await?;
                self.stdin.write(b"\n").await?;
                self.line_input.clear();
                self.cursor = 0;
            }
            _ => {}
        }
        Ok(vec![])
    }
}

fn char_to_buf (c: char) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut arr = [0; 4];
    let bytes = c.encode_utf8(&mut arr).as_bytes();
    for byte in bytes {
        if *byte == 0 {
            break;
        }
        buf.push(*byte); 
    }
    buf
}
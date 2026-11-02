//! Running the game and streaming its output.

use std::process::{ExitStatus, Stdio};

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use crate::{Error, LaunchCommand, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    pub stream: LogStream,
    pub text: String,
}

pub struct GameProcess {
    child: Child,
    lines: mpsc::UnboundedReceiver<LogLine>,
}

/// Starts the game. On Windows no console window is opened.
pub fn spawn(command: &LaunchCommand) -> Result<GameProcess> {
    let mut cmd = Command::new(&command.program);
    cmd.args(&command.args)
        .current_dir(&command.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(false);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().map_err(|source| Error::Spawn { path: command.program.clone(), source })?;
    let (tx, lines) = mpsc::unbounded_channel();
    if let Some(out) = child.stdout.take() {
        forward(out, LogStream::Stdout, tx.clone());
    }
    if let Some(err) = child.stderr.take() {
        forward(err, LogStream::Stderr, tx);
    }
    Ok(GameProcess { child, lines })
}

fn forward(reader: impl AsyncRead + Unpin + Send + 'static, stream: LogStream, tx: mpsc::UnboundedSender<LogLine>) {
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(text)) = lines.next_line().await {
            if tx.send(LogLine { stream, text }).is_err() {
                break;
            }
        }
    });
}

impl GameProcess {
    pub fn id(&self) -> Option<u32> {
        self.child.id()
    }

    /// Delivers every log line to `on_line` and returns when the game exits.
    pub async fn wait(mut self, mut on_line: impl FnMut(LogLine)) -> Result<ExitStatus> {
        while let Some(line) = self.lines.recv().await {
            on_line(line);
        }
        self.child.wait().await.map_err(|source| Error::Io { path: "game process".into(), source })
    }

    pub async fn kill(&mut self) -> Result<()> {
        self.child.kill().await.map_err(|source| Error::Io { path: "game process".into(), source })
    }
}

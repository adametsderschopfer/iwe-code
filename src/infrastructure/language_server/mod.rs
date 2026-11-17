//! Run language servers through their standard input and output streams.

pub(crate) mod client;
pub(crate) mod discovery;
mod environment;
pub(crate) mod service;
mod transport;

use serde_json::{Value, json};
use std::{io, path::Path, process::Stdio, time::Duration};
use tokio::{
    io::BufReader,
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::mpsc,
};

pub(crate) struct Session {
    child: Child,
    input: ChildStdin,
    messages: mpsc::Receiver<io::Result<Value>>,
    reader: tokio::task::JoinHandle<()>,
    next_request: u64,
}

impl Session {
    pub(crate) fn start(program: &Path, arguments: &[String], root: &Path) -> io::Result<Self> {
        let mut child = Command::new(program)
            .args(arguments)
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(if cfg!(test) {
                Stdio::inherit()
            } else {
                Stdio::null()
            })
            .kill_on_drop(true)
            .spawn()?;
        let input = child.stdin.take().ok_or(io::ErrorKind::BrokenPipe)?;
        let output = child.stdout.take().ok_or(io::ErrorKind::BrokenPipe)?;
        let (sender, messages) = mpsc::channel(128);
        let reader = tokio::spawn(read_messages(output, sender));
        Ok(Self {
            child,
            input,
            messages,
            reader,
            next_request: 0,
        })
    }

    pub(crate) async fn notify(&mut self, method: &str, params: Value) -> io::Result<()> {
        self.send(json!({"jsonrpc": "2.0", "method": method, "params": params}))
            .await
    }

    pub(crate) async fn request(&mut self, method: &str, params: Value) -> io::Result<u64> {
        self.next_request += 1;
        let id = self.next_request;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .await?;
        Ok(id)
    }

    pub(crate) async fn respond(&mut self, id: Value, result: Value) -> io::Result<()> {
        self.send(json!({"jsonrpc": "2.0", "id": id, "result": result}))
            .await
    }

    pub(crate) async fn reject(&mut self, id: Value) -> io::Result<()> {
        self.send(json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": "Method not supported"}})).await
    }

    pub(crate) async fn receive(&mut self) -> io::Result<Value> {
        self.messages
            .recv()
            .await
            .ok_or(io::ErrorKind::UnexpectedEof)?
    }

    async fn send(&mut self, message: Value) -> io::Result<()> {
        tokio::time::timeout(
            Duration::from_secs(10),
            transport::write(&mut self.input, &message),
        )
        .await
        .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))?
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.reader.abort();
        let _ = self.child.start_kill();
    }
}

async fn read_messages(output: ChildStdout, sender: mpsc::Sender<io::Result<Value>>) {
    let mut reader = BufReader::new(output);
    loop {
        let message = transport::read(&mut reader).await;
        let failed = message.is_err();
        if sender.send(message).await.is_err() || failed {
            break;
        }
    }
}

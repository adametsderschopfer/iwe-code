//! Run a project shell through the operating system's pseudo-terminal.

#[cfg(unix)]
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::{
    fmt,
    io::{Read, Write},
    path::Path,
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
#[cfg(windows)]
use xpty::{CommandBuilder, PtySize, PtySystem, native_pty_system};

pub(crate) const MAX_INPUT: usize = 64 * 1024;

#[derive(Clone, Default)]
pub(crate) struct Lifetime {
    cancel: CancellationToken,
    children: TaskTracker,
}

impl Lifetime {
    pub(super) fn tab(&self) -> Self {
        Self {
            cancel: self.cancel.child_token(),
            children: self.children.clone(),
        }
    }

    pub(super) fn stop(&self) {
        self.cancel.cancel();
    }

    pub(crate) fn stopping(&self) -> bool {
        self.children.is_closed()
    }

    /// Stop every child and report when the last child exits.
    pub(crate) fn shutdown(self) -> Option<impl Future<Output = ()> + Send + 'static> {
        self.stop();
        self.children.close();
        (!self.children.is_empty()).then_some(async move { self.children.wait().await })
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Output {
    Bytes(Vec<u8>),
    Closed,
    Failed,
}

enum Control {
    Input(Vec<u8>),
    Resize(u16, u16),
    Close,
}

pub(crate) struct Session {
    controls: mpsc::SyncSender<Control>,
    output: tokio::sync::Mutex<tokio::sync::mpsc::Receiver<Output>>,
    stop: mpsc::Sender<()>,
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session").finish_non_exhaustive()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.stop.send(());
    }
}

impl Session {
    pub(crate) fn start(
        directory: &Path,
        rows: u16,
        cols: u16,
        lifetime: Lifetime,
    ) -> Result<Arc<Self>, ()> {
        let command = CommandBuilder::new_default_prog();
        Self::spawn(directory, rows, cols, command, lifetime)
    }

    pub(super) fn program(
        directory: &Path,
        path: &Path,
        args: Vec<String>,
        lifetime: Lifetime,
    ) -> Result<Arc<Self>, ()> {
        let mut command = CommandBuilder::new(path);
        command.args(args);
        Self::spawn(directory, 24, 80, command, lifetime)
    }

    fn spawn(
        directory: &Path,
        rows: u16,
        cols: u16,
        mut command: CommandBuilder,
        lifetime: Lifetime,
    ) -> Result<Arc<Self>, ()> {
        let child_worker = lifetime.children.token();
        if lifetime.cancel.is_cancelled() || !directory.is_dir() {
            return Err(());
        }
        let pair = native_pty_system()
            .openpty(size(rows, cols))
            .map_err(|_| ())?;
        let mut reader = pair.master.try_clone_reader().map_err(|_| ())?;
        let mut writer = pair.master.take_writer().map_err(|_| ())?;
        command.cwd(directory);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        let mut child = pair.slave.spawn_command(command).map_err(|_| ())?;
        drop(pair.slave);

        let (controls, requests) = mpsc::sync_channel(64);
        let shutdown = controls.clone();
        let (stop, stopped) = mpsc::channel();
        let mut killer = child.clone_killer();
        if thread::Builder::new()
            .name("terminal-lifecycle".into())
            .spawn(move || {
                loop {
                    match stopped.recv_timeout(Duration::from_millis(100)) {
                        Err(mpsc::RecvTimeoutError::Timeout) if !lifetime.cancel.is_cancelled() => {
                            match child.try_wait() {
                                Ok(None) => {}
                                _ => break,
                            }
                        }
                        _ => {
                            let _ = child.kill();
                            let _ = child.wait();
                            break;
                        }
                    }
                }
                drop(child_worker);
                // Release ConPTY on the input thread while the output thread drains it.
                let _ = shutdown.send(Control::Close);
            })
            .is_err()
        {
            let _ = killer.kill();
            return Err(());
        }

        let (events, output) = tokio::sync::mpsc::channel(64);
        let session = Arc::new(Self {
            controls,
            output: tokio::sync::Mutex::new(output),
            stop,
        });
        let failure = events.clone();
        thread::Builder::new()
            .name("terminal-input".into())
            .spawn(move || {
                while let Ok(control) = requests.recv() {
                    let result = match control {
                        Control::Close => break,
                        Control::Input(bytes) => {
                            writer.write_all(&bytes).and_then(|()| writer.flush())
                        }
                        Control::Resize(rows, cols) => pair
                            .master
                            .resize(size(rows, cols))
                            .map_err(std::io::Error::other),
                    };
                    if result.is_err() {
                        let _ = failure.blocking_send(Output::Failed);
                        break;
                    }
                }
            })
            .map_err(|_| ())?;
        thread::Builder::new()
            .name("terminal-output".into())
            .spawn(move || {
                let mut bytes = [0; 4096];
                loop {
                    match reader.read(&mut bytes) {
                        Ok(0) => break,
                        Ok(count) => {
                            if events
                                .blocking_send(Output::Bytes(bytes[..count].to_vec()))
                                .is_err()
                            {
                                return;
                            }
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                        // Unix pseudo-terminals may signal their final close as EIO.
                        Err(error) if cfg!(unix) && error.raw_os_error() == Some(5) => break,
                        Err(_) => {
                            let _ = events.blocking_send(Output::Failed);
                            return;
                        }
                    }
                }
                let _ = events.blocking_send(Output::Closed);
            })
            .map_err(|_| ())?;
        Ok(session)
    }

    pub(crate) fn input(&self, bytes: Vec<u8>) -> Result<(), ()> {
        if bytes.len() > MAX_INPUT {
            return Err(());
        }
        self.send_control(Control::Input(bytes))
    }

    pub(crate) fn resize(&self, rows: u16, cols: u16) -> Result<(), ()> {
        self.send_control(Control::Resize(rows, cols))
    }

    fn send_control(&self, control: Control) -> Result<(), ()> {
        match self.controls.try_send(control) {
            // The output worker delivers the final status after the input worker closes.
            Ok(()) | Err(mpsc::TrySendError::Disconnected(_)) => Ok(()),
            Err(mpsc::TrySendError::Full(_)) => Err(()),
        }
    }

    pub(crate) async fn next(&self) -> Vec<Output> {
        let mut output = self.output.lock().await;
        let Some(first) = output.recv().await else {
            return vec![Output::Closed];
        };
        let mut events = vec![first];
        while events.len() < 32 {
            match output.try_recv() {
                Ok(event) => events.push(event),
                Err(_) => break,
            }
        }
        events
    }
}

fn size(rows: u16, cols: u16) -> PtySize {
    PtySize {
        rows: rows.clamp(1, 200),
        cols: cols.clamp(1, 400),
        pixel_width: 0,
        pixel_height: 0,
    }
}

#[cfg(test)]
mod tests;

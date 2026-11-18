//! Single-instance IPC using local sockets/named pipes.

use crossbeam_channel::{Receiver, Sender};
use interprocess::local_socket::{
    prelude::*, GenericNamespaced, ListenerNonblockingMode, ListenerOptions, Stream,
};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Default named pipe / socket base name for Just IDE single-instance coordination.
#[must_use]
pub fn default_pipe_name() -> String {
    let user = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "user".to_string());
    let sanitized: String = user
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("JustIDE-{sanitized}")
}

/// Payload sent from a secondary instance to the primary instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcPayload {
    /// Command-line arguments from the secondary instance.
    pub args: Vec<String>,
    /// Working directory of the secondary instance.
    pub cwd: Option<PathBuf>,
    /// Whether the secondary instance requests focus.
    pub request_focus: bool,
}

/// Attempts to send a payload to an existing instance.
///
/// Returns `true` if a running instance acknowledged the payload, `false` if no instance is running.
#[must_use]
pub fn try_send_to_existing(pipe_name: &str, payload: &IpcPayload) -> bool {
    let Ok(name) = pipe_name.to_ns_name::<GenericNamespaced>() else {
        return false;
    };

    let Ok(mut stream) = Stream::connect(name) else {
        return false;
    };

    let _ = stream.set_recv_timeout(Some(Duration::from_millis(500)));
    let _ = stream.set_send_timeout(Some(Duration::from_millis(500)));

    let Ok(json) = serde_json::to_string(payload) else {
        return false;
    };

    if stream.write_all(json.as_bytes()).is_err()
        || stream.write_all(b"\n").is_err()
        || stream.flush().is_err()
    {
        return false;
    }

    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    matches!(reader.read_line(&mut response), Ok(n) if n > 0 && response.trim() == "ok")
}

/// IPC server that listens for payloads from secondary instances.
pub struct IpcServer {
    receiver: Receiver<IpcPayload>,
    shutdown: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl IpcServer {
    /// Starts an IPC server listening on the given pipe/socket name.
    ///
    /// Returns `None` if the socket cannot be bound.
    #[must_use]
    pub fn start(pipe_name: &str) -> Option<Self> {
        let name = pipe_name.to_ns_name::<GenericNamespaced>().ok()?;

        let listener = ListenerOptions::new().name(name).create_sync().ok()?;
        let _ = listener.set_nonblocking(ListenerNonblockingMode::Accept);

        let (tx, rx) = crossbeam_channel::unbounded();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_flag = Arc::clone(&shutdown);

        let handle = thread::Builder::new()
            .name("jide-ipc".to_string())
            .spawn(move || {
                while !shutdown_flag.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok(stream) => {
                            let _ = stream.set_nonblocking(false);
                            let tx = tx.clone();
                            thread::spawn(move || {
                                let _ = handle_client(stream, &tx);
                            });
                        }
                        Err(e) if e.kind() == ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(20));
                        }
                        Err(_) => {
                            if shutdown_flag.load(Ordering::Relaxed) {
                                break;
                            }
                            thread::sleep(Duration::from_millis(50));
                        }
                    }
                }
            })
            .ok()?;

        Some(Self {
            receiver: rx,
            shutdown,
            handle: Some(handle),
        })
    }

    /// Returns a receiver for incoming payloads.
    #[must_use]
    pub fn receiver(&self) -> &Receiver<IpcPayload> {
        &self.receiver
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn handle_client(mut stream: Stream, tx: &Sender<IpcPayload>) -> std::io::Result<()> {
    let mut reader = BufReader::new(&mut stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;

    let payload: IpcPayload = serde_json::from_str(line.trim())
        .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e))?;

    let _ = tx.send(payload);
    stream.write_all(b"ok\n")?;
    stream.flush()?;
    Ok(())
}

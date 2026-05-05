//! Unix domain socket client for attached mode.
//!
//! When the EcoCode CLI is running (sudo ecocode -- python train.py), it injects
//! ECOCODE_SOCKET into the child process environment. This client connects to that
//! socket and sends newline-delimited JSON marker events to the CLI.
//!
//! Design principles:
//! - Thread-safe: Mutex guards every write.
//! - Never crashes the caller: all errors are silently swallowed after the first failure.
//! - Retries connection up to 3 times with 50ms spacing.

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

pub struct SocketClient {
    stream: Mutex<Option<UnixStream>>,
}

impl SocketClient {
    /// Try to connect to the CLI's Unix socket, retrying up to 3 times.
    /// Returns a SocketClient that silently no-ops if connection fails.
    pub fn new(path: &str) -> (Self, Option<String>) {
        for attempt in 0..3 {
            match UnixStream::connect(path) {
                Ok(stream) => {
                    return (
                        Self {
                            stream: Mutex::new(Some(stream)),
                        },
                        None,
                    );
                }
                Err(_) => {
                    if attempt < 2 {
                        thread::sleep(Duration::from_millis(50));
                    }
                }
            }
        }
        let warning = format!(
            "[ecocode] Could not connect to CLI socket at {}. \
             Markers will be dropped. Is the CLI running with -o online?",
            path
        );
        (
            Self {
                stream: Mutex::new(None),
            },
            Some(warning),
        )
    }

    /// Send a JSON event over the socket. Silently no-ops on failure.
    pub fn send_event(&self, event: &serde_json::Value) {
        let mut guard = self.stream.lock().unwrap();
        if let Some(ref mut stream) = *guard {
            let mut msg = serde_json::to_string(event).unwrap_or_default();
            msg.push('\n');
            if stream.write_all(msg.as_bytes()).is_err() {
                // Connection broken — stop future attempts silently.
                *guard = None;
            }
        }
    }

    /// Close the socket connection.
    pub fn close(&self) {
        let mut guard = self.stream.lock().unwrap();
        if let Some(stream) = guard.take() {
            drop(stream);
        }
    }
}

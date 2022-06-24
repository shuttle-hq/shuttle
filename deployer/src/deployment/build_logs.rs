use super::{BuildLogReceiver, BuildLogSender};

use std::collections::HashMap;
use std::io;
use std::sync::Arc;

use tokio::sync::{mpsc, Mutex};

const BUFFER_SIZE: usize = 10;

#[derive(Clone)]
pub struct BuildLogsManager {
    receivers: Arc<Mutex<BuildLogReceivers>>,
}

impl BuildLogsManager {
    pub fn new() -> Self {
        BuildLogsManager {
            receivers: Arc::new(Mutex::new(BuildLogReceivers::new())),
        }
    }

    pub async fn for_deployment(&self, name: String) -> BuildLogWriter {
        let (sender, receiver) = mpsc::channel(BUFFER_SIZE);

        // TODO: Handle case where deployment already exists in map.
        self.receivers.lock().await.insert(name, receiver);

        BuildLogWriter {
            sender,
            buffer: String::new(),
        }
    }

    pub async fn take_receiver(&self, name: &str) -> Option<BuildLogReceiver> {
        self.receivers.lock().await.remove(name)
    }
}

type BuildLogReceivers = HashMap<String, BuildLogReceiver>;

pub struct BuildLogWriter {
    sender: BuildLogSender,
    buffer: String,
}

impl io::Write for BuildLogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for c in buf {
            let c = *c as char;

            if c == '\n' {
                self.flush()?;
            } else {
                self.buffer.push(c);
            }
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let sender = self.sender.clone();
        let msg = self.buffer.clone();

        self.buffer.clear();

        // Work around the fact that this function does not return a future
        // meaning `sender.send` can't be used, but is also executing in Tokio
        // context meaning `blocking_send` panics. Spawning and immediately
        // joining a thread 'escapes' Tokio meaning `blocking_send` can be used.
        std::thread::spawn(move || {
            let _ = sender.blocking_send(msg);
        })
        .join()
        .unwrap();

        Ok(())
    }
}

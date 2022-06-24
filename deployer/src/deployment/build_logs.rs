use super::{BuildLogReceiver, BuildLogSender};

use std::collections::HashMap;
use std::io;
use std::sync::Arc;

use tokio::sync::{broadcast, Mutex};

const BUFFER_SIZE: usize = 200;

#[derive(Clone)]
pub struct BuildLogsManager {
    receivers: Arc<Mutex<HashMap<String, PastLogsReceiverPair>>>,
}

impl BuildLogsManager {
    pub fn new() -> Self {
        BuildLogsManager {
            receivers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn for_deployment(&self, name: String) -> BuildLogWriter {
        let (sender, receiver) = broadcast::channel(BUFFER_SIZE);

        let sender_clone = sender.clone();
        self.receivers.lock().await.insert(
            name,
            PastLogsReceiverPair {
                sender: sender_clone,
                original_receiver: receiver,
                logs_consumed_so_far: Vec::new(),
            },
        );

        BuildLogWriter {
            sender,
            buffer: String::new(),
        }
    }

    pub async fn take_receiver(&self, name: &str) -> Option<BuildLogReceiver> {
        self.receivers
            .lock()
            .await
            .get(name)
            .map(|p| p.sender.subscribe())
    }

    pub async fn get_logs_so_far(&self, name: &str) -> Vec<String> {
        let mut new_lines = Vec::new();

        if let Some(receiver) = self
            .receivers
            .lock()
            .await
            .get_mut(name)
            .map(|p| &mut p.original_receiver)
        {
            while let Ok(line) = receiver.try_recv() {
                new_lines.push(line);
            }
        }

        if let Some(deployment) = self.receivers.lock().await.get_mut(name) {
            deployment.logs_consumed_so_far.extend(new_lines);
            deployment.logs_consumed_so_far.clone()
        } else {
            Vec::new()
        }
    }
}

struct PastLogsReceiverPair {
    sender: BuildLogSender,
    original_receiver: BuildLogReceiver,
    logs_consumed_so_far: Vec<String>,
}

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
            let _ = sender.send(msg);
        })
        .join()
        .unwrap();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn abc() {}
}

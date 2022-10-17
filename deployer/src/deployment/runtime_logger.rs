use shuttle_common::LogItem;
use shuttle_service::Logger;
use tokio::sync::mpsc::{self, UnboundedSender};
use uuid::Uuid;

use super::deploy_layer::{self, LogType};

pub trait Factory: Send + 'static {
    fn get_logger(&self, id: Uuid) -> Logger;
}

/// Factory to create runtime loggers for deployments
pub struct RuntimeLoggerFactory {
    log_send: crossbeam_channel::Sender<deploy_layer::Log>,
}

impl RuntimeLoggerFactory {
    pub fn new(log_send: crossbeam_channel::Sender<deploy_layer::Log>) -> Self {
        Self { log_send }
    }
}

impl Factory for RuntimeLoggerFactory {
    fn get_logger(&self, id: Uuid) -> Logger {
        let (tx, mut rx): (UnboundedSender<LogItem>, _) = mpsc::unbounded_channel();

        let sender = self.log_send.clone();

        tokio::spawn(async move {
            while let Some(log) = rx.recv().await {
                sender.send(log.into()).expect("to send log to persistence");
            }
        });

        Logger::new(tx, id)
    }
}

impl From<LogItem> for deploy_layer::Log {
    fn from(log: LogItem) -> Self {
        Self {
            id: log.id,
            state: log.state.into(),
            level: log.level.into(),
            timestamp: log.timestamp,
            file: log.file,
            line: log.line,
            target: log.target,
            fields: serde_json::from_slice(&log.fields).unwrap(),
            r#type: LogType::Event,
            address: None,
        }
    }
}

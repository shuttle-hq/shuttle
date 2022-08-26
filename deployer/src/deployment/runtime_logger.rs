use chrono::Utc;
use log::{Level, Metadata, Record};
use serde_json::json;
use uuid::Uuid;

use crate::persistence::{LogLevel, State};

use super::deploy_layer;

pub trait Factory: Send + 'static {
    fn get_logger(&self, id: Uuid) -> Box<dyn log::Log>;
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
    fn get_logger(&self, id: Uuid) -> Box<dyn log::Log> {
        Box::new(RuntimeLogger::new(id, self.log_send.clone()))
    }
}

/// Captures and redirects runtime logs for a deploy
/// TODO: convert to a tracing subscriber
pub struct RuntimeLogger {
    id: Uuid,
    log_send: crossbeam_channel::Sender<deploy_layer::Log>,
}

impl RuntimeLogger {
    pub(crate) fn new(id: Uuid, log_send: crossbeam_channel::Sender<deploy_layer::Log>) -> Self {
        Self { id, log_send }
    }
}

impl log::Log for RuntimeLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let datetime = Utc::now();

            self.log_send
                .send(deploy_layer::Log {
                    id: self.id,
                    state: State::Running,
                    level: record.level().into(),
                    timestamp: datetime,
                    file: record.file().map(String::from),
                    line: record.line(),
                    target: record.target().to_string(),
                    fields: json!({
                        "message": format!("{}", record.args()),
                    }),
                    r#type: deploy_layer::LogType::Event,
                })
                .expect("sending log should succeed");
        }
    }

    fn flush(&self) {}
}

impl From<Level> for LogLevel {
    fn from(level: Level) -> Self {
        match level {
            Level::Error => Self::Error,
            Level::Warn => Self::Warn,
            Level::Info => Self::Info,
            Level::Debug => Self::Debug,
            Level::Trace => Self::Trace,
        }
    }
}

use chrono::Utc;
use log::{Level, Metadata, Record};
use serde_json::json;
use shuttle_common::{deployment::State, LogItem};
use uuid::Uuid;

pub struct Logger {
    id: Uuid,
}

impl Logger {
    pub fn new() -> Self {
        Self {
            id: Default::default(),
        }
    }
}

impl log::Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            // Reuse LogItem from common to have the same output as runtime logs from production
            let item = LogItem {
                id: self.id,
                state: State::Running,
                level: get_level(record.level()),
                timestamp: Utc::now(),
                file: record.file().map(String::from),
                line: record.line(),
                target: record.target().to_string(),
                fields: json!({
                    "message": format!("{}", record.args()),
                }),
            };

            println!("{item}");
        }
    }

    fn flush(&self) {}
}

fn get_level(level: Level) -> shuttle_common::log::Level {
    match level {
        Level::Error => shuttle_common::log::Level::Error,
        Level::Warn => shuttle_common::log::Level::Warn,
        Level::Info => shuttle_common::log::Level::Info,
        Level::Debug => shuttle_common::log::Level::Debug,
        Level::Trace => shuttle_common::log::Level::Trace,
    }
}

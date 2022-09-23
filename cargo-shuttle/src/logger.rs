use std::{collections::HashMap, env, str::FromStr};

use chrono::Utc;
use log::{Level, Metadata, ParseLevelError, Record};
use serde_json::json;
use shuttle_common::{deployment::State, LogItem};
use uuid::Uuid;

pub struct Logger {
    id: Uuid,
    filter: HashMap<String, Level>,
}

impl Logger {
    pub fn new() -> Self {
        let filter = if let Ok(rust_log) = env::var("RUST_LOG") {
            let rust_log = rust_log
                .split(',')
                .map(|item| {
                    // Try to get target and level if both are set
                    if let Some((target, level)) = item.split_once('=') {
                        Result::<(String, Level), ParseLevelError>::Ok((
                            target.to_string(),
                            Level::from_str(level)?,
                        ))
                    } else {
                        // Ok only target or level is set, but which is it
                        if let Ok(level) = Level::from_str(item) {
                            Ok((String::new(), level))
                        } else {
                            Ok((item.to_string(), Level::Trace))
                        }
                    }
                })
                .filter_map(Result::ok);

            HashMap::from_iter(rust_log)
        } else {
            HashMap::from([(String::new(), Level::Error)])
        };

        Self {
            id: Default::default(),
            filter,
        }
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        for (target, level) in self.filter.iter() {
            if metadata.target().starts_with(target) && &metadata.level() <= level {
                return true;
            }
        }

        false
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

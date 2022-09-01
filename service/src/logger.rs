use std::{collections::HashMap, env, str::FromStr};

use chrono::{DateTime, Utc};
use log::{Level, Metadata, ParseLevelError, Record};
use shuttle_common::{DeploymentId, LogItem};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug)]
pub struct Log {
    pub deployment_id: DeploymentId,
    pub datetime: DateTime<Utc>,
    pub item: LogItem,
}

pub struct Logger {
    deployment_id: DeploymentId,
    tx: UnboundedSender<Log>,
    filter: HashMap<String, Level>,
}

impl Logger {
    pub fn new(tx: UnboundedSender<Log>, deployment_id: DeploymentId) -> Self {
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
            tx,
            deployment_id,
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
            let datetime = Utc::now();
            let item = LogItem {
                body: format!("{}", record.args()),
                level: record.level(),
                target: record.target().to_string(),
            };

            self.tx
                .send(Log {
                    item,
                    datetime,
                    deployment_id: self.deployment_id,
                })
                .expect("sending log should succeed");
        }
    }

    fn flush(&self) {}
}

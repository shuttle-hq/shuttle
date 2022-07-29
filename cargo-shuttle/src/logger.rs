use chrono::Utc;
use log::{Metadata, Record};
use shuttle_common::LogItem;

use crate::print;

pub struct Logger;

impl log::Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let datetime = Utc::now();
            let item = LogItem {
                body: format!("{}", record.args()),
                level: record.level(),
                target: record.target().to_string(),
            };

            print::log(datetime, item);
        }
    }

    fn flush(&self) {}
}

use chrono::Utc;
use crossterm::style::{StyledContent, Stylize};
use log::{Level, Metadata, Record};

pub struct Logger;

impl log::Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let datetime = Utc::now();

            println!(
                "{}{} {:<5} {}{} {}",
                "[".dark_grey(),
                datetime.format("%Y-%m-%dT%H:%M:%SZ"),
                get_colored_level(&record.level()),
                record.target().to_string(),
                "]".dark_grey(),
                format!("{}", record.args())
            );
        }
    }

    fn flush(&self) {}
}

fn get_colored_level(level: &Level) -> StyledContent<String> {
    match level {
        Level::Trace => level.to_string().dark_grey(),
        Level::Debug => level.to_string().blue(),
        Level::Info => level.to_string().green(),
        Level::Warn => level.to_string().yellow(),
        Level::Error => level.to_string().red(),
    }
}

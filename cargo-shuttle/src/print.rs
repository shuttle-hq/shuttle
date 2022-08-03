use chrono::{DateTime, Local, Utc};
use crossterm::style::{StyledContent, Stylize};
use log::Level;
use shuttle_common::LogItem;

pub fn log(datetime: DateTime<Utc>, log_item: LogItem) {
    let datetime: DateTime<Local> = DateTime::from(datetime);
    println!(
        "{}{} {:<5} {}{} {}",
        "[".dark_grey(),
        datetime.format("%Y-%m-%dT%H:%M:%SZ"),
        get_colored_level(&log_item.level),
        log_item.target,
        "]".dark_grey(),
        log_item.body
    );
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

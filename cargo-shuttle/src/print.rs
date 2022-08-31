use chrono::{DateTime, Local, Utc};
use colored::{ColoredString, Colorize};
use shuttle_common::LogItem;

pub fn log(datetime: DateTime<Utc>, log_item: LogItem) {
    let datetime: DateTime<Local> = DateTime::from(datetime);
    println!(
        "{}{} {:<5} {}{} {}",
        "[".bright_black(),
        datetime.format("%Y-%m-%dT%H:%M:%SZ"),
        get_colored_level(&log_item.level),
        log_item.target,
        "]".bright_black(),
        log_item.body
    );
}

// fn get_colored_level(level: &Level) -> ColoredString {
fn get_colored_level(level: &String) -> ColoredString {
    level.green()
    // match level {
    //     Level::Trace => level.to_string().bright_black(),
    //     Level::Debug => level.to_string().blue(),
    //     Level::Info => level.to_string().green(),
    //     Level::Warn => level.to_string().yellow(),
    //     Level::Error => level.to_string().red(),
    // }
}

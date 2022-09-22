use chrono::{DateTime, Local, Utc};
use colored::{ColoredString, Colorize};
use serde_json::json;
use shuttle_common::LogItem;

pub fn log(datetime: DateTime<Utc>, log_item: LogItem) {
    let datetime: DateTime<Local> = DateTime::from(datetime);

    let mut fields: serde_json::Map<String, serde_json::Value> =
        serde_json::from_slice(&log_item.fields).unwrap();

    let message = fields.remove("message").unwrap_or_else(|| json!(""));

    println!(
        "{}{} {:<5} {}{} {} {}",
        "[".bright_black(),
        datetime.format("%Y-%m-%dT%H:%M:%SZ").to_string().dimmed(),
        get_colored_level(&log_item.level),
        log_item.target.dimmed(),
        "]".bright_black(),
        message.as_str().unwrap(),
        fmt_fields(&fields).dimmed()
    );
}

fn get_colored_level(level: &String) -> ColoredString {
    match &level.to_uppercase()[..] {
        "TRACE" => level.bright_black(),
        "DEBUG" => level.blue(),
        "INFO" => level.green(),
        "WARN" => level.yellow(),
        "ERROR" => level.red(),
        _ => level.bright_black(), // TODO: should this panic?
    }
}

fn fmt_fields(fields: &serde_json::Map<String, serde_json::Value>) -> String {
    fields
        .iter()
        .map(|(field, value)| {
            // display strings without quotes
            let value = if value.is_string() {
                value.as_str().unwrap().to_owned()
            } else {
                value.to_string()
            };

            format!("{}={}", field.italic(), value)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

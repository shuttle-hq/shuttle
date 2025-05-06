use chrono::{DateTime, Utc};
#[cfg(feature = "display")]
use crossterm::style::Stylize;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct LogItem {
    pub timestamp: DateTime<Utc>,
    /// Which container / log stream this line came from
    pub source: String,
    pub line: String,
}

impl LogItem {
    pub fn new(timestamp: DateTime<Utc>, source: String, line: String) -> Self {
        Self {
            timestamp,
            source,
            line,
        }
    }
}

#[cfg(feature = "display")]
impl std::fmt::Display for LogItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let datetime: chrono::DateTime<chrono::Local> = DateTime::from(self.timestamp);

        write!(
            f,
            "{} [{}] {}",
            datetime
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, false)
                .dim(),
            self.source,
            self.line,
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct LogsResponse {
    pub logs: Vec<LogItem>,
}

#[cfg(test)]
mod tests {
    #[cfg_attr(not(feature = "display"), allow(unused_imports))]
    use super::*;

    // Chrono uses std Time (to libc) internally, if you want to use this method
    // in more than one test, you need to handle async tests properly.
    #[cfg(feature = "display")]
    fn with_tz<F: FnOnce()>(tz: &str, f: F) {
        let prev_tz = std::env::var("TZ").unwrap_or_default();
        std::env::set_var("TZ", tz);
        f();
        std::env::set_var("TZ", prev_tz);
    }

    #[cfg(feature = "display")]
    #[rstest::rstest]
    #[case::utc("utc")]
    #[case::cest("cest")]
    fn timezone_formatting(#[case] tz: &str) {
        let item = LogItem::new(
            Utc::now(),
            "test".to_string(),
            r#"{"message": "Building"}"#.to_owned(),
        );

        with_tz(tz, || {
            let value = item
                .timestamp
                .with_timezone(&chrono::Local)
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, false);

            let log_line = format!("{}", &item);

            assert!(log_line.contains(&value));
        });
    }
}

use chrono::{DateTime, Utc};
#[cfg(feature = "display")]
use crossterm::style::{StyledContent, Stylize};
use serde::{Deserialize, Serialize};
use strum::EnumString;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Clone, Debug, EnumString, Eq, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "display", derive(strum::Display))]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub enum InternalLogOrigin {
    Unknown,
    Deployer,
    // Builder,
    // ResourceRecorder,
}

impl Default for InternalLogOrigin {
    fn default() -> Self {
        Self::Unknown
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[cfg_attr(feature = "openapi", schema(as = shuttle_common::log::Item))]
pub struct Item {
    #[cfg_attr(feature = "openapi", schema(value_type = KnownFormat::Uuid))]
    pub id: Uuid,
    #[cfg_attr(feature = "openapi", schema(value_type = shuttle_common::log::InternalLogOrigin))]
    pub internal_origin: InternalLogOrigin,
    #[cfg_attr(feature = "openapi", schema(value_type = KnownFormat::DateTime))]
    pub timestamp: DateTime<Utc>,
    pub line: String,
    // #[cfg_attr(feature = "openapi", schema(value_type = shuttle_common::deployment::State))]
    // pub state: State,
    // #[cfg_attr(feature = "openapi", schema(value_type = shuttle_common::log::Level))]
    // pub level: Level,
    // pub file: Option<String>,
    // pub line: Option<u32>,
    // pub target: String,
    // pub fields: Vec<u8>,
}

#[cfg(feature = "display")]
impl std::fmt::Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let datetime: chrono::DateTime<chrono::Local> = DateTime::from(self.timestamp);

        write!(
            f,
            "{} [{}] {}",
            datetime.to_rfc3339().dim(),
            self.internal_origin,
            self.line,
        )
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[cfg_attr(feature = "openapi", schema(as = shuttle_common::log::Level))]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[cfg(feature = "display")]
impl Level {
    fn get_colored(&self) -> StyledContent<&str> {
        match self {
            Level::Trace => "TRACE".magenta(),
            Level::Debug => "DEBUG".blue(),
            Level::Info => " INFO".green(),
            Level::Warn => " WARN".yellow(),
            Level::Error => "ERROR".red(),
        }
    }
}

impl From<&tracing::Level> for Level {
    fn from(level: &tracing::Level) -> Self {
        match *level {
            tracing::Level::ERROR => Self::Error,
            tracing::Level::WARN => Self::Warn,
            tracing::Level::INFO => Self::Info,
            tracing::Level::DEBUG => Self::Debug,
            tracing::Level::TRACE => Self::Trace,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Chrono uses std Time (to libc) internally, if you want to use this method
    // in more than one test, you need to handle async tests properly.
    fn with_tz<F: FnOnce()>(tz: &str, f: F) {
        let prev_tz = std::env::var("TZ").unwrap_or("".to_string());
        std::env::set_var("TZ", tz);
        f();
        std::env::set_var("TZ", prev_tz);
    }

    #[test]
    fn test_timezone_formatting() {
        let item = Item {
            id: Uuid::new_v4(),
            internal_origin: InternalLogOrigin::Deployer,
            timestamp: Utc::now(),
            line: r#"{"message": "Building"}"#.to_owned(),
        };

        with_tz("CEST", || {
            let cest_dt = item.timestamp.with_timezone(&chrono::Local).to_rfc3339();
            let log_line = format!("{}", &item);

            assert!(log_line.contains(&cest_dt));
        });

        with_tz("UTC", || {
            let utc_dt = item.timestamp.with_timezone(&chrono::Local).to_rfc3339();
            let log_line = format!("{}", &item);

            assert!(log_line.contains(&utc_dt));
        });
    }
}

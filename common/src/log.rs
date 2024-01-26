use std::fmt::Write;

use chrono::{DateTime, Utc};
#[cfg(feature = "display")]
use crossterm::style::StyledContent;
#[cfg(feature = "display")]
use crossterm::style::Stylize;
use serde::{Deserialize, Serialize};
use strum::EnumString;
use tracing::{field::Visit, span, warn, Event, Level, Metadata, Subscriber};
use tracing_subscriber::Layer;
use uuid::Uuid;

use crate::tracing::JsonVisitor;

/// Used to determine settings based on which backend crate does what
#[derive(Clone, Debug, Default, EnumString, Eq, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "display", derive(strum::Display))]
pub enum Backend {
    /// Is considered an error
    #[default]
    Unknown,

    Auth,
    Builder,
    Deployer,
    Gateway,
    Logger,
    Provisioner,
    ResourceRecorder,
    Runtime(String),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LogItem {
    /// Deployment id
    pub id: Uuid,

    /// Internal service that produced this log
    pub internal_origin: Backend,

    /// Time log was captured
    pub timestamp: DateTime<Utc>,

    /// The log line
    pub line: String,
}

const LOGLINE_MAX_CHARS: usize = 2048;
const TRUNC_MSG: &str = "... (truncated)";

impl LogItem {
    pub fn new(id: Uuid, internal_origin: Backend, line: impl Into<String>) -> Self {
        let mut line: String = line.into();

        Self::truncate_line(&mut line);

        Self {
            id,
            internal_origin,
            timestamp: Utc::now(),
            line,
        }
    }

    fn truncate_line(line: &mut String) {
        // Check if it can be over the limit (assuming ascii only), no iteration
        if line.len() > LOGLINE_MAX_CHARS {
            // Then, check if it actually is over the limit.
            // Find the char boundary of the last char, but iterate no more than
            // the max number of chars allowed.
            let x = line
                .char_indices()
                .enumerate()
                .find(|(i, _)| *i == LOGLINE_MAX_CHARS);
            // If char iterator reached max iteration count
            if let Some((_, (ci, _))) = x {
                // Truncate to the char boundary found
                line.truncate(ci);
                // New allocation unlikely since it keeps its capacity
                write!(line, "{}", TRUNC_MSG).expect("write to string");
            }
        }
    }

    pub fn get_raw_line(&self) -> &str {
        &self.line
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
            self.internal_origin,
            self.line,
        )
    }
}

#[cfg(feature = "display")]
pub trait ColoredLevel {
    fn colored(&self) -> StyledContent<&str>;
}

#[cfg(feature = "display")]
impl ColoredLevel for tracing::Level {
    fn colored(&self) -> StyledContent<&str> {
        match *self {
            Level::TRACE => "TRACE".magenta(),
            Level::DEBUG => "DEBUG".blue(),
            Level::INFO => " INFO".green(),
            Level::WARN => " WARN".yellow(),
            Level::ERROR => "ERROR".red(),
        }
    }
}

#[cfg(feature = "display")]
pub fn format_event(event: &Event<'_>) -> String {
    let metadata = event.metadata();
    let mut visitor = JsonVisitor::default();
    event.record(&mut visitor);

    let mut message = String::new();

    let target = visitor
        .target
        .unwrap_or_else(|| metadata.target().to_string());

    if !target.is_empty() {
        let t = format!("{target}: ").dim();
        write!(message, "{t}").unwrap();
    }

    let mut simple = None;
    let mut extra = vec![];
    for (key, value) in visitor.fields.iter() {
        match key.as_str() {
            "message" => simple = value.as_str(),
            _ => extra.push(format!("{key}={value}")),
        }
    }
    if !extra.is_empty() {
        write!(message, "{{{}}} ", extra.join(" ")).unwrap();
    }
    if let Some(msg) = simple {
        write!(message, "{msg}").unwrap();
    }

    format!("{} {}", metadata.level().colored(), message)
}

/// Records logs for the deployment progress
pub trait LogRecorder: Clone + Send + 'static {
    fn record(&self, log: LogItem);
}

/// Tracing subscriber layer which logs based on if the log
/// is from a span that is tagged with a deployment id
pub struct DeploymentLogLayer<R>
where
    R: LogRecorder + Send + Sync,
{
    pub log_recorder: R,
    pub internal_service: Backend,
}

impl<R, S> Layer<S> for DeploymentLogLayer<R>
where
    S: Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
    R: LogRecorder + Send + Sync + 'static,
{
    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        // We only care about events in some scope
        let scope = if let Some(scope) = ctx.event_scope(event) {
            scope
        } else {
            return;
        };

        // Find the outermost scope with the scope details containing the current deployment id
        for span in scope.from_root() {
            let extensions = span.extensions();

            if let Some(details) = extensions.get::<ScopeDetails>() {
                self.log_recorder.record(LogItem::new(
                    details.deployment_id,
                    self.internal_service.clone(),
                    format_event(event),
                ));
                break;
            }
        }
    }
    fn on_new_span(
        &self,
        attrs: &span::Attributes<'_>,
        id: &span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // We only care about spans that concern a deployment
        if !DeploymentIdVisitor::is_valid(attrs.metadata()) {
            return;
        }
        let mut visitor = DeploymentIdVisitor::default();
        attrs.record(&mut visitor);
        let details = visitor.details;

        if details.deployment_id.is_nil() {
            warn!("scope details does not have a valid deployment_id");
            return;
        }

        // Safe to unwrap since this is the `on_new_span` method
        let span = ctx.span(id).unwrap();
        let mut extensions = span.extensions_mut();

        let metadata = attrs.metadata();

        let message = format!("{} {}", metadata.level().colored(), metadata.name().blue());

        self.log_recorder.record(LogItem::new(
            details.deployment_id,
            self.internal_service.clone(),
            message,
        ));

        extensions.insert::<ScopeDetails>(details);
    }
}

#[derive(Debug, Default)]
struct ScopeDetails {
    deployment_id: Uuid,
}
/// To extract `deployment_id` field for scopes that have it
#[derive(Default)]
struct DeploymentIdVisitor {
    details: ScopeDetails,
}

impl DeploymentIdVisitor {
    /// Field containing the deployment identifier
    const ID_IDENT: &'static str = "deployment_id";

    fn is_valid(metadata: &Metadata) -> bool {
        metadata.is_span() && metadata.fields().field(Self::ID_IDENT).is_some()
    }
}

impl Visit for DeploymentIdVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == Self::ID_IDENT {
            self.details.deployment_id = Uuid::try_parse(&format!("{value:?}")).unwrap_or_default();
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
        let item = LogItem::new(
            Uuid::new_v4(),
            Backend::Deployer,
            r#"{"message": "Building"}"#.to_owned(),
        );

        with_tz("CEST", || {
            let cest_dt = item
                .timestamp
                .with_timezone(&chrono::Local)
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, false);
            let log_line = format!("{}", &item);

            assert!(log_line.contains(&cest_dt));
        });

        with_tz("UTC", || {
            let utc_dt = item
                .timestamp
                .with_timezone(&chrono::Local)
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, false);
            let log_line = format!("{}", &item);

            assert!(log_line.contains(&utc_dt));
        });
    }

    #[test]
    fn log_item_truncate() {
        let mut l = "öl".repeat(100);
        LogItem::truncate_line(&mut l);
        assert_eq!(l.len(), 300);
        assert_eq!(l.chars().count(), 200);

        let mut l = "🍪".repeat(LOGLINE_MAX_CHARS);
        LogItem::truncate_line(&mut l);
        assert_eq!(l.len(), 4 * (LOGLINE_MAX_CHARS));
        assert_eq!(l.chars().count(), LOGLINE_MAX_CHARS);

        // one cookie should be truncated, and the suffix message should be appended
        // ✨ = 3b, 🍪 = 4b

        let mut l = format!("A{}", "🍪".repeat(LOGLINE_MAX_CHARS));
        LogItem::truncate_line(&mut l);
        assert_eq!(l.len(), 1 + 4 * (LOGLINE_MAX_CHARS - 1) + TRUNC_MSG.len());
        assert_eq!(l.chars().count(), LOGLINE_MAX_CHARS + TRUNC_MSG.len());

        let mut l = format!("✨{}", "🍪".repeat(LOGLINE_MAX_CHARS));
        LogItem::truncate_line(&mut l);
        assert_eq!(l.len(), 3 + 4 * (LOGLINE_MAX_CHARS - 1) + TRUNC_MSG.len());
        assert_eq!(l.chars().count(), LOGLINE_MAX_CHARS + TRUNC_MSG.len());
    }
}

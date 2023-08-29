use chrono::{DateTime, Utc};
#[cfg(feature = "display")]
use crossterm::style::Stylize;
use serde::{Deserialize, Serialize};
use strum::EnumString;
use tracing::{field::Visit, span, warn, Metadata, Subscriber};
use tracing_subscriber::Layer;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;
use uuid::Uuid;

/// Used to determine settings based on which backend crate does what
#[derive(Clone, Debug, EnumString, Eq, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "display", derive(strum::Display))]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub enum Backend {
    /// Is considered an error
    Unknown,

    Auth,
    // Builder,
    Deployer,
    Gateway,
    Logger,
    Provisioner,
    ResourceRecorder,
}

impl Default for Backend {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[cfg_attr(feature = "openapi", schema(as = shuttle_common::log::LogItem))]
pub struct LogItem {
    /// Deployment id
    #[cfg_attr(feature = "openapi", schema(value_type = KnownFormat::Uuid))]
    pub id: Uuid,

    /// Internal service that produced this log
    #[cfg_attr(feature = "openapi", schema(value_type = shuttle_common::log::InternalLogOrigin))]
    pub internal_origin: Backend,

    /// Time log was produced
    #[cfg_attr(feature = "openapi", schema(value_type = KnownFormat::DateTime))]
    pub timestamp: DateTime<Utc>,

    /// The log line
    pub line: String,
}

#[cfg(feature = "display")]
impl std::fmt::Display for LogItem {
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
    pub recorder: R,
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

        // Find the outermost scope with the scope details containing the current state
        for span in scope.from_root() {
            let extensions = span.extensions();

            if let Some(details) = extensions.get::<ScopeDetails>() {
                self.recorder.record(LogItem {
                    id: details.id,
                    internal_origin: self.internal_service.clone(),
                    timestamp: Utc::now(),
                    line: "Test".into(),
                });
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
        // We only care about spans that change the state
        if !DeploymentIdVisitor::is_valid(attrs.metadata()) {
            return;
        }
        let mut visitor = DeploymentIdVisitor::default();
        attrs.record(&mut visitor);
        let details = visitor.details;

        if details.id.is_nil() {
            warn!("scope details does not have a valid id");
            return;
        }

        // Safe to unwrap since this is the `on_new_span` method
        let span = ctx.span(id).unwrap();
        let mut extensions = span.extensions_mut();

        self.recorder.record(LogItem {
            id: details.id,
            internal_origin: self.internal_service.clone(),
            timestamp: Utc::now(),
            line: "Test".into(),
        });

        extensions.insert::<ScopeDetails>(details);
    }
}

#[derive(Debug, Default)]
struct ScopeDetails {
    id: Uuid,
}
/// To extract `id` field for scopes that have it
#[derive(Default)]
struct DeploymentIdVisitor {
    details: ScopeDetails,
}

impl DeploymentIdVisitor {
    /// Field containing the deployment identifier
    const ID_IDENT: &'static str = "id";

    fn is_valid(metadata: &Metadata) -> bool {
        metadata.is_span() && metadata.fields().field(Self::ID_IDENT).is_some()
    }
}

impl Visit for DeploymentIdVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == Self::ID_IDENT {
            self.details.id = Uuid::try_parse(&format!("{value:?}")).unwrap_or_default();
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
        let item = LogItem {
            id: Uuid::new_v4(),
            internal_origin: Backend::Deployer,
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

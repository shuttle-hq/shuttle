//! This is a layer for [tracing] to capture the state transition of deploys
//!
//! The idea is as follow: as a deployment moves through the [super::DeploymentManager] a set of functions will be invoked.
//! These functions are clear markers for the deployment entering a new state so we would want to change the state as soon as entering these functions.
//! But rather than passing a persistence layer around to be able record the state in these functions we can rather use [tracing].
//!
//! This is very similar to Aspect Oriented Programming where we use the annotations from the function to trigger the recording of a new state.
//! This annotation is a [#[instrument]](https://docs.rs/tracing-attributes/latest/tracing_attributes/attr.instrument.html) with a `name` and `state` field as follow:
//! ```
//! #[instrument(fields(name = built.name.as_str(), state = %State::Built))]
//! pub async fn new_state_fn(built: Built) {
//!     // Get built ready for starting
//! }
//! ```
//!
//! Here the `name` is extracted from the `built` argument and the `state` is taken from the [State] enum (the special `%` is needed to use the `Display` trait to convert it to a string).
//!
//! All `debug!()` etc in these functions will be captured by this layer and will be associated with the deployment and the state.
//!
//! **Warning** Don't log out sensitive info in functions with these annotations

use chrono::{DateTime, Utc};
use serde_json::json;
use tracing::{field::Visit, span, Metadata, Subscriber};
use tracing_subscriber::Layer;

use super::{
    log::{self, Level},
    DeploymentInfo, State,
};

/// Records logs for the deployment progress
pub trait LogRecorder {
    fn record(&self, log: Log);
}

/// An event or state transition log
#[derive(Debug, PartialEq)]
pub struct Log {
    /// Deployment name
    pub name: String,

    /// Current state of the deployment
    pub state: State,

    /// Log level
    pub level: Level,

    /// Time log happened
    pub timestamp: DateTime<Utc>,

    /// File event took place in
    pub file: Option<String>,

    /// Line in file event happened on
    pub line: Option<u32>,

    /// Extra structured log fields
    pub fields: serde_json::Value,

    pub r#type: LogType,
}

impl From<Log> for log::Log {
    fn from(log: Log) -> Self {
        Self {
            name: log.name,
            timestamp: log.timestamp,
            state: log.state,
            level: log.level,
            file: log.file,
            line: log.line,
            fields: log.fields,
        }
    }
}

impl From<Log> for DeploymentInfo {
    fn from(log: Log) -> Self {
        Self {
            name: log.name,
            state: log.state,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum LogType {
    Event,
    State,
}

/// Tracing subscriber layer which keeps track of a deployment's state
pub struct DeployLayer<R>
where
    R: LogRecorder + Send + Sync,
{
    recorder: R,
}

impl<R> DeployLayer<R>
where
    R: LogRecorder + Send + Sync,
{
    pub fn new(recorder: R) -> Self {
        Self { recorder }
    }
}

impl<R, S> Layer<S> for DeployLayer<R>
where
    S: Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
    R: LogRecorder + Send + Sync + 'static,
{
    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        // We only care about events in some state scope
        let scope = if let Some(scope) = ctx.event_scope(event) {
            scope
        } else {
            return;
        };

        // Find the first scope with the scope details containing the current state
        for span in scope.from_root() {
            let extensions = span.extensions();

            if let Some(details) = extensions.get::<ScopeDetails>() {
                let mut visitor = JsonVisitor::default();

                event.record(&mut visitor);
                let metadata = event.metadata();

                self.recorder.record(Log {
                    name: details.name.clone(),
                    state: details.state,
                    level: metadata.level().into(),
                    timestamp: Utc::now(),
                    file: metadata.file().map(str::to_string),
                    line: metadata.line(),
                    fields: serde_json::Value::Object(visitor.0),
                    r#type: LogType::Event,
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
        if !NewStateVisitor::is_valid(attrs.metadata()) {
            return;
        }

        let mut visitor = NewStateVisitor::default();

        attrs.record(&mut visitor);

        let details = visitor.details;

        // Safe to unwrap since this is the `on_new_span` method
        let span = ctx.span(id).unwrap();
        let mut extensions = span.extensions_mut();
        let metadata = span.metadata();

        self.recorder.record(Log {
            name: details.name.clone(),
            state: details.state,
            level: metadata.level().into(),
            timestamp: Utc::now(),
            file: metadata.file().map(str::to_string),
            line: metadata.line(),
            fields: Default::default(),
            r#type: LogType::State,
        });

        extensions.insert::<ScopeDetails>(details);
    }
}

/// Used to keep track of the current state a deployment scope is in
#[derive(Debug, Default)]
struct ScopeDetails {
    name: String,
    state: State,
}

impl From<&tracing::Level> for Level {
    fn from(level: &tracing::Level) -> Self {
        match level {
            &tracing::Level::TRACE => Self::Trace,
            &tracing::Level::DEBUG => Self::Debug,
            &tracing::Level::INFO => Self::Info,
            &tracing::Level::WARN => Self::Warn,
            &tracing::Level::ERROR => Self::Error,
        }
    }
}

/// This visitor is meant to extract the `ScopeDetails` for any scope with `name` and `status` fields
#[derive(Default)]
struct NewStateVisitor {
    details: ScopeDetails,
}

impl NewStateVisitor {
    /// Field containing the deployment name identifier
    const NAME_IDENT: &'static str = "name";

    /// Field containing the deployment state identifier
    const STATE_IDENT: &'static str = "state";

    fn is_valid(metadata: &Metadata) -> bool {
        metadata.is_span()
            && metadata.fields().field(Self::NAME_IDENT).is_some()
            && metadata.fields().field(Self::STATE_IDENT).is_some()
    }
}

impl Visit for NewStateVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == Self::NAME_IDENT {
            self.details.name = value.to_string();
        }
    }
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == Self::STATE_IDENT {
            self.details.state = value.into();
        }
    }
}

#[derive(Default)]
struct JsonVisitor(serde_json::Map<String, serde_json::Value>);

impl Visit for JsonVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0.insert(field.name().to_string(), json!(value));
    }
    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.0.insert(field.name().to_string(), json!(value));
    }
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.0.insert(field.name().to_string(), json!(value));
    }
    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.0.insert(field.name().to_string(), json!(value));
    }
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.0.insert(field.name().to_string(), json!(value));
    }
    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.0
            .insert(field.name().to_string(), json!(value.to_string()));
    }
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0
            .insert(field.name().to_string(), json!(format!("{value:?}")));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::body::Bytes;
    use ctor::ctor;
    use futures::FutureExt;
    use tracing_subscriber::prelude::*;

    use crate::deployment::{deploy_layer::LogType, Built, DeploymentManager, Queued, State};

    use super::{DeployLayer, Log, LogRecorder};

    #[ctor]
    static RECORDER: Arc<Mutex<RecorderMock>> = {
        let recorder = RecorderMock::new();
        tracing_subscriber::registry()
            .with(DeployLayer::new(Arc::clone(&recorder)))
            .init();

        recorder
    };

    struct RecorderMock {
        states: Arc<Mutex<Vec<StateLog>>>,
    }

    #[derive(Clone, Debug, PartialEq)]
    struct StateLog {
        name: String,
        state: State,
    }

    impl From<Log> for StateLog {
        fn from(log: Log) -> Self {
            Self {
                name: log.name,
                state: log.state,
            }
        }
    }

    impl RecorderMock {
        fn new() -> Arc<Mutex<Self>> {
            Arc::new(Mutex::new(Self {
                states: Arc::new(Mutex::new(Vec::new())),
            }))
        }

        fn get_deployment_states(&self, name: &str) -> Vec<StateLog> {
            self.states
                .lock()
                .unwrap()
                .iter()
                .filter(|log| log.name == name)
                .cloned()
                .collect()
        }
    }

    impl LogRecorder for RecorderMock {
        fn record(&self, event: Log) {
            // We are only testing the state transitions
            if event.r#type == LogType::State {
                self.states.lock().unwrap().push(event.into());
            }
        }
    }

    impl<R: LogRecorder> LogRecorder for Arc<Mutex<R>> {
        fn record(&self, event: Log) {
            self.lock().unwrap().record(event);
        }
    }

    #[tokio::test]
    async fn deployment_to_be_queued() {
        let deployment_manager = DeploymentManager::new();

        deployment_manager
            .queue_push(Queued {
                name: "queue_test".to_string(),
                data_stream: Box::pin(async { Ok(Bytes::from("data")) }.into_stream()),
            })
            .await;

        // Give it a small time to start up
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let recorder = RECORDER.lock().unwrap();
        let states = recorder.get_deployment_states("queue_test");

        assert_eq!(
            states.len(),
            4,
            "did not expect these states:\n\t{states:#?}"
        );

        assert_eq!(
            *states,
            vec![
                StateLog {
                    name: "queue_test".to_string(),
                    state: State::Queued,
                },
                StateLog {
                    name: "queue_test".to_string(),
                    state: State::Building,
                },
                StateLog {
                    name: "queue_test".to_string(),
                    state: State::Built,
                },
                StateLog {
                    name: "queue_test".to_string(),
                    state: State::Running,
                },
            ]
        );
    }

    #[tokio::test]
    async fn deployment_from_run() {
        let deployment_manager = DeploymentManager::new();

        deployment_manager
            .run_push(Built {
                name: "run_test".to_string(),
            })
            .await;

        // Give it a small time to start up
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let recorder = RECORDER.lock().unwrap();
        let states = recorder.get_deployment_states("run_test");

        assert_eq!(
            states.len(),
            2,
            "did not expect these states:\n\t{states:#?}"
        );

        assert_eq!(
            *states,
            vec![
                StateLog {
                    name: "run_test".to_string(),
                    state: State::Built,
                },
                StateLog {
                    name: "run_test".to_string(),
                    state: State::Running,
                },
            ]
        );
    }
}

//! This is a layer for [tracing] to capture the state transition of deploys
//!
//! The idea is as follow: as a deployment moves through the [super::DeploymentManager] a set of functions will be invoked.
//! These functions are clear markers for the deployment entering a new state so we would want to change the state as soon as entering these functions.
//! But rather than passing a persistence layer around to be able record the state in these functions we can rather use [tracing].
//!
//! This is very similar to Aspect Oriented Programming where we use the annotations from the function to trigger the recording of a new state.
//! This annotation is a [#[instrument]](https://docs.rs/tracing-attributes/latest/tracing_attributes/attr.instrument.html) with an `id` and `state` field as follow:
//! ```no-test
//! #[instrument(fields(id = %built.id, state = %State::Built))]
//! pub async fn new_state_fn(built: Built) {
//!     // Get built ready for starting
//! }
//! ```
//!
//! Here the `id` is extracted from the `built` argument and the `state` is taken from the [State] enum (the special `%` is needed to use the `Display` trait to convert the values to a str).
//!
//! All `debug!()` etc in these functions will be captured by this layer and will be associated with the deployment and the state.
//!
//! **Warning** Don't log out sensitive info in functions with these annotations

use chrono::{DateTime, Utc};
use serde_json::json;
use shuttle_common::{tracing::JsonVisitor, STATE_MESSAGE};
use shuttle_proto::runtime;
use std::{str::FromStr, time::SystemTime};
use tracing::{field::Visit, span, warn, Metadata, Subscriber};
use tracing_subscriber::Layer;
use uuid::Uuid;

use crate::persistence::{self, DeploymentState, LogLevel, State};

/// Records logs for the deployment progress
pub trait LogRecorder: Clone + Send + 'static {
    fn record(&self, log: Log);
}

/// An event or state transition log
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Log {
    /// Deployment id
    pub id: Uuid,

    /// Current state of the deployment
    pub state: State,

    /// Log level
    pub level: LogLevel,

    /// Time log happened
    pub timestamp: DateTime<Utc>,

    /// File event took place in
    pub file: Option<String>,

    /// Line in file event happened on
    pub line: Option<u32>,

    /// Module log took place in
    pub target: String,

    /// Extra structured log fields
    pub fields: serde_json::Value,

    pub r#type: LogType,
}

impl From<Log> for persistence::Log {
    fn from(log: Log) -> Self {
        // Make sure state message is set for state logs
        // This is used to know when the end of the build logs has been reached
        let fields = match log.r#type {
            LogType::Event => log.fields,
            LogType::State => json!(STATE_MESSAGE),
        };

        Self {
            id: log.id,
            timestamp: log.timestamp,
            state: log.state,
            level: log.level,
            file: log.file,
            line: log.line,
            target: log.target,
            fields,
        }
    }
}

impl From<Log> for shuttle_common::LogItem {
    fn from(log: Log) -> Self {
        Self {
            id: log.id,
            timestamp: log.timestamp,
            state: log.state.into(),
            level: log.level.into(),
            file: log.file,
            line: log.line,
            target: log.target,
            fields: serde_json::to_vec(&log.fields).unwrap(),
        }
    }
}

impl From<Log> for DeploymentState {
    fn from(log: Log) -> Self {
        Self {
            id: log.id,
            state: log.state,
            last_update: log.timestamp,
        }
    }
}

impl From<runtime::LogItem> for Log {
    fn from(log: runtime::LogItem) -> Self {
        Self {
            id: Uuid::from_slice(&log.id).unwrap(),
            state: runtime::LogState::from_i32(log.state).unwrap().into(),
            level: runtime::LogLevel::from_i32(log.level).unwrap().into(),
            timestamp: DateTime::from(SystemTime::try_from(log.timestamp.unwrap()).unwrap()),
            file: log.file,
            line: log.line,
            target: log.target,
            fields: serde_json::from_slice(&log.fields).unwrap(),
            r#type: LogType::Event,
        }
    }
}

impl From<runtime::LogState> for State {
    fn from(state: runtime::LogState) -> Self {
        match state {
            runtime::LogState::Queued => Self::Queued,
            runtime::LogState::Building => Self::Building,
            runtime::LogState::Built => Self::Built,
            runtime::LogState::Loading => Self::Loading,
            runtime::LogState::Running => Self::Running,
            runtime::LogState::Completed => Self::Completed,
            runtime::LogState::Stopped => Self::Stopped,
            runtime::LogState::Crashed => Self::Crashed,
            runtime::LogState::Unknown => Self::Unknown,
        }
    }
}

impl From<runtime::LogLevel> for LogLevel {
    fn from(level: runtime::LogLevel) -> Self {
        match level {
            runtime::LogLevel::Trace => Self::Trace,
            runtime::LogLevel::Debug => Self::Debug,
            runtime::LogLevel::Info => Self::Info,
            runtime::LogLevel::Warn => Self::Warn,
            runtime::LogLevel::Error => Self::Error,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
                    id: details.id,
                    state: details.state,
                    level: metadata.level().into(),
                    timestamp: Utc::now(),
                    file: visitor.file.or_else(|| metadata.file().map(str::to_string)),
                    line: visitor.line.or_else(|| metadata.line()),
                    target: visitor
                        .target
                        .unwrap_or_else(|| metadata.target().to_string()),
                    fields: serde_json::Value::Object(visitor.fields),
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

        if details.id.is_nil() {
            warn!("scope details does not have a valid id");
            return;
        }

        // Safe to unwrap since this is the `on_new_span` method
        let span = ctx.span(id).unwrap();
        let mut extensions = span.extensions_mut();
        let metadata = span.metadata();

        self.recorder.record(Log {
            id: details.id,
            state: details.state,
            level: metadata.level().into(),
            timestamp: Utc::now(),
            file: metadata.file().map(str::to_string),
            line: metadata.line(),
            target: metadata.target().to_string(),
            fields: Default::default(),
            r#type: LogType::State,
        });

        extensions.insert::<ScopeDetails>(details);
    }
}

/// Used to keep track of the current state a deployment scope is in
#[derive(Debug, Default)]
struct ScopeDetails {
    id: Uuid,
    state: State,
}

impl From<&tracing::Level> for LogLevel {
    fn from(level: &tracing::Level) -> Self {
        match *level {
            tracing::Level::TRACE => Self::Trace,
            tracing::Level::DEBUG => Self::Debug,
            tracing::Level::INFO => Self::Info,
            tracing::Level::WARN => Self::Warn,
            tracing::Level::ERROR => Self::Error,
        }
    }
}

/// This visitor is meant to extract the `ScopeDetails` for any scope with `name` and `status` fields
#[derive(Default)]
struct NewStateVisitor {
    details: ScopeDetails,
}

impl NewStateVisitor {
    /// Field containing the deployment identifier
    const ID_IDENT: &'static str = "id";

    /// Field containing the deployment state identifier
    const STATE_IDENT: &'static str = "state";

    fn is_valid(metadata: &Metadata) -> bool {
        metadata.is_span()
            && metadata.fields().field(Self::ID_IDENT).is_some()
            && metadata.fields().field(Self::STATE_IDENT).is_some()
    }
}

impl Visit for NewStateVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == Self::STATE_IDENT {
            self.details.state = State::from_str(&format!("{value:?}")).unwrap_or_default();
        } else if field.name() == Self::ID_IDENT {
            self.details.id = Uuid::try_parse(&format!("{value:?}")).unwrap_or_default();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs::read_dir,
        net::{Ipv4Addr, SocketAddr},
        path::PathBuf,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use crate::{persistence::DeploymentUpdater, RuntimeManager};
    use async_trait::async_trait;
    use axum::body::Bytes;
    use ctor::ctor;
    use flate2::{write::GzEncoder, Compression};
    use portpicker::pick_unused_port;
    use shuttle_proto::provisioner::{
        provisioner_server::{Provisioner, ProvisionerServer},
        DatabaseRequest, DatabaseResponse,
    };
    use tempdir::TempDir;
    use tokio::{select, time::sleep};
    use tonic::transport::Server;
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};
    use uuid::Uuid;

    use crate::{
        deployment::{
            deploy_layer::LogType, gateway_client::BuildQueueClient, ActiveDeploymentsGetter,
            Built, DeploymentManager, Queued,
        },
        persistence::{Secret, SecretGetter, SecretRecorder, State},
    };

    use super::{DeployLayer, Log, LogRecorder};

    #[ctor]
    static RECORDER: Arc<Mutex<RecorderMock>> = {
        let recorder = RecorderMock::new();

        // Copied from the test-log crate
        let event_filter = {
            use ::tracing_subscriber::fmt::format::FmtSpan;

            match ::std::env::var("RUST_LOG_SPAN_EVENTS") {
          Ok(value) => {
            value
              .to_ascii_lowercase()
              .split(',')
              .map(|filter| match filter.trim() {
                "new" => FmtSpan::NEW,
                "enter" => FmtSpan::ENTER,
                "exit" => FmtSpan::EXIT,
                "close" => FmtSpan::CLOSE,
                "active" => FmtSpan::ACTIVE,
                "full" => FmtSpan::FULL,
                _ => panic!("test-log: RUST_LOG_SPAN_EVENTS must contain filters separated by `,`.\n\t\
                  For example: `active` or `new,close`\n\t\
                  Supported filters: new, enter, exit, close, active, full\n\t\
                  Got: {}", value),
              })
              .fold(FmtSpan::NONE, |acc, filter| filter | acc)
          },
          Err(::std::env::VarError::NotUnicode(_)) =>
            panic!("test-log: RUST_LOG_SPAN_EVENTS must contain a valid UTF-8 string"),
          Err(::std::env::VarError::NotPresent) => FmtSpan::NONE,
        }
        };
        let fmt_layer = fmt::layer()
            .with_test_writer()
            .with_span_events(event_filter);
        let filter_layer = EnvFilter::from_default_env();

        tracing_subscriber::registry()
            .with(DeployLayer::new(Arc::clone(&recorder)))
            .with(filter_layer)
            .with(fmt_layer)
            .init();

        recorder
    };

    #[derive(Clone)]
    struct RecorderMock {
        states: Arc<Mutex<Vec<StateLog>>>,
    }

    #[derive(Clone, Debug, PartialEq)]
    struct StateLog {
        id: Uuid,
        state: State,
    }

    impl From<Log> for StateLog {
        fn from(log: Log) -> Self {
            Self {
                id: log.id,
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

        fn get_deployment_states(&self, id: &Uuid) -> Vec<StateLog> {
            self.states
                .lock()
                .unwrap()
                .iter()
                .filter(|log| log.id == *id)
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

    struct ProvisionerMock;

    #[async_trait]
    impl Provisioner for ProvisionerMock {
        async fn provision_database(
            &self,
            _request: tonic::Request<DatabaseRequest>,
        ) -> Result<tonic::Response<DatabaseResponse>, tonic::Status> {
            panic!("no run tests should request a db");
        }
    }

    fn get_runtime_manager() -> Arc<tokio::sync::Mutex<RuntimeManager>> {
        let provisioner_addr =
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), pick_unused_port().unwrap());
        let mock = ProvisionerMock;

        tokio::spawn(async move {
            Server::builder()
                .add_service(ProvisionerServer::new(mock))
                .serve(provisioner_addr)
                .await
                .unwrap();
        });

        let tmp_dir = TempDir::new("shuttle_run_test").unwrap();
        let path = tmp_dir.into_path();
        let (tx, _rx) = crossbeam_channel::unbounded();

        RuntimeManager::new(path, format!("http://{}", provisioner_addr), tx)
    }

    #[async_trait::async_trait]
    impl SecretRecorder for Arc<Mutex<RecorderMock>> {
        type Err = std::io::Error;

        async fn insert_secret(
            &self,
            _service_id: &Uuid,
            _key: &str,
            _value: &str,
        ) -> Result<(), Self::Err> {
            panic!("no tests should set secrets")
        }
    }

    impl<R: LogRecorder> LogRecorder for Arc<Mutex<R>> {
        fn record(&self, event: Log) {
            self.lock().unwrap().record(event);
        }
    }

    #[derive(Clone)]
    struct StubDeploymentUpdater;

    #[async_trait::async_trait]
    impl DeploymentUpdater for StubDeploymentUpdater {
        type Err = std::io::Error;

        async fn set_address(&self, _id: &Uuid, _address: &SocketAddr) -> Result<(), Self::Err> {
            Ok(())
        }

        async fn set_is_next(&self, _id: &Uuid, _is_next: bool) -> Result<(), Self::Err> {
            Ok(())
        }
    }

    #[derive(Clone)]
    struct StubActiveDeploymentGetter;

    #[async_trait::async_trait]
    impl ActiveDeploymentsGetter for StubActiveDeploymentGetter {
        type Err = std::io::Error;

        async fn get_active_deployments(
            &self,
            _service_id: &Uuid,
        ) -> std::result::Result<Vec<Uuid>, Self::Err> {
            Ok(vec![])
        }
    }

    #[derive(Clone)]
    struct StubBuildQueueClient;

    #[async_trait::async_trait]
    impl BuildQueueClient for StubBuildQueueClient {
        async fn get_slot(
            &self,
            _id: Uuid,
        ) -> Result<bool, crate::deployment::gateway_client::Error> {
            Ok(true)
        }

        async fn release_slot(
            &self,
            _id: Uuid,
        ) -> Result<(), crate::deployment::gateway_client::Error> {
            Ok(())
        }
    }

    #[derive(Clone)]
    struct StubSecretGetter;

    #[async_trait::async_trait]
    impl SecretGetter for StubSecretGetter {
        type Err = std::io::Error;

        async fn get_secrets(&self, _service_id: &Uuid) -> Result<Vec<Secret>, Self::Err> {
            Ok(Default::default())
        }
    }

    async fn test_states(id: &Uuid, expected_states: Vec<StateLog>) {
        loop {
            let states = RECORDER.lock().unwrap().get_deployment_states(id);

            if *states == expected_states {
                break;
            }

            sleep(Duration::from_millis(250)).await;
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn deployment_to_be_queued() {
        let deployment_manager = get_deployment_manager().await;

        let queued = get_queue("sleep-async");
        let id = queued.id;
        deployment_manager.queue_push(queued).await;

        let test = test_states(
            &id,
            vec![
                StateLog {
                    id,
                    state: State::Queued,
                },
                StateLog {
                    id,
                    state: State::Building,
                },
                StateLog {
                    id,
                    state: State::Built,
                },
                StateLog {
                    id,
                    state: State::Loading,
                },
                StateLog {
                    id,
                    state: State::Running,
                },
            ],
        );

        select! {
            _ = sleep(Duration::from_secs(180)) => {
                let states = RECORDER.lock().unwrap().get_deployment_states(&id);
                panic!("states should go into 'Running' for a valid service: {:#?}", states);
            },
            _ = test => {}
        };

        // Send kill signal
        deployment_manager.kill(id).await;

        sleep(Duration::from_secs(1)).await;

        let states = RECORDER.lock().unwrap().get_deployment_states(&id);

        assert_eq!(
            *states,
            vec![
                StateLog {
                    id,
                    state: State::Queued,
                },
                StateLog {
                    id,
                    state: State::Building,
                },
                StateLog {
                    id,
                    state: State::Built,
                },
                StateLog {
                    id,
                    state: State::Loading,
                },
                StateLog {
                    id,
                    state: State::Running,
                },
                StateLog {
                    id,
                    state: State::Stopped,
                },
            ]
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn deployment_self_stop() {
        let deployment_manager = get_deployment_manager().await;

        let queued = get_queue("self-stop");
        let id = queued.id;
        deployment_manager.queue_push(queued).await;

        let test = test_states(
            &id,
            vec![
                StateLog {
                    id,
                    state: State::Queued,
                },
                StateLog {
                    id,
                    state: State::Building,
                },
                StateLog {
                    id,
                    state: State::Built,
                },
                StateLog {
                    id,
                    state: State::Loading,
                },
                StateLog {
                    id,
                    state: State::Running,
                },
                StateLog {
                    id,
                    state: State::Completed,
                },
            ],
        );

        select! {
            _ = sleep(Duration::from_secs(180)) => {
                let states = RECORDER.lock().unwrap().get_deployment_states(&id);
                panic!("states should go into 'Completed' when a service stops by itself: {:#?}", states);
            }
            _ = test => {}
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn deployment_bind_panic() {
        let deployment_manager = get_deployment_manager().await;

        let queued = get_queue("bind-panic");
        let id = queued.id;
        deployment_manager.queue_push(queued).await;

        let test = test_states(
            &id,
            vec![
                StateLog {
                    id,
                    state: State::Queued,
                },
                StateLog {
                    id,
                    state: State::Building,
                },
                StateLog {
                    id,
                    state: State::Built,
                },
                StateLog {
                    id,
                    state: State::Loading,
                },
                StateLog {
                    id,
                    state: State::Running,
                },
                StateLog {
                    id,
                    state: State::Crashed,
                },
            ],
        );

        select! {
            _ = sleep(Duration::from_secs(180)) => {
                let states = RECORDER.lock().unwrap().get_deployment_states(&id);
                panic!("states should go into 'Crashed' panicing in bind: {:#?}", states);
            }
            _ = test => {}
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn deployment_main_panic() {
        let deployment_manager = get_deployment_manager().await;

        let queued = get_queue("main-panic");
        let id = queued.id;
        deployment_manager.queue_push(queued).await;

        let test = test_states(
            &id,
            vec![
                StateLog {
                    id,
                    state: State::Queued,
                },
                StateLog {
                    id,
                    state: State::Building,
                },
                StateLog {
                    id,
                    state: State::Built,
                },
                StateLog {
                    id,
                    state: State::Loading,
                },
                StateLog {
                    id,
                    state: State::Running,
                },
                StateLog {
                    id,
                    state: State::Crashed,
                },
            ],
        );

        select! {
            _ = sleep(Duration::from_secs(180)) => {
                let states = RECORDER.lock().unwrap().get_deployment_states(&id);
                panic!("states should go into 'Crashed' when panicing in main: {:#?}", states);
            }
            _ = test => {}
        }
    }

    #[tokio::test]
    async fn deployment_from_run() {
        let deployment_manager = get_deployment_manager().await;

        let id = Uuid::new_v4();
        deployment_manager
            .run_push(Built {
                id,
                service_name: "run-test".to_string(),
                service_id: Uuid::new_v4(),
                tracing_context: Default::default(),
                is_next: false,
            })
            .await;

        let test = test_states(
            &id,
            vec![
                StateLog {
                    id,
                    state: State::Built,
                },
                StateLog {
                    id,
                    state: State::Loading,
                },
                StateLog {
                    id,
                    state: State::Crashed,
                },
            ],
        );

        select! {
            _ = sleep(Duration::from_secs(30)) => {
                let states = RECORDER.lock().unwrap().get_deployment_states(&id);
                panic!("from running should start in built and end in crash for invalid: {:#?}", states)
            },
            _ = test => {}
        };
    }

    #[tokio::test]
    async fn scope_with_nil_id() {
        let deployment_manager = get_deployment_manager().await;

        let id = Uuid::nil();
        deployment_manager
            .queue_push(Queued {
                id,
                service_name: "nil_id".to_string(),
                service_id: Uuid::new_v4(),
                data: Bytes::from("violets are red").to_vec(),
                will_run_tests: false,
                tracing_context: Default::default(),
            })
            .await;

        // Give it a small time to start up
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let recorder = RECORDER.lock().unwrap();
        let states = recorder.get_deployment_states(&id);

        assert!(
            states.is_empty(),
            "no logs should be recorded when the scope id is invalid:\n\t{states:#?}"
        );
    }

    async fn get_deployment_manager() -> DeploymentManager {
        DeploymentManager::builder()
            .build_log_recorder(RECORDER.clone())
            .secret_recorder(RECORDER.clone())
            .active_deployment_getter(StubActiveDeploymentGetter)
            .artifacts_path(PathBuf::from("/tmp"))
            .secret_getter(StubSecretGetter)
            .runtime(get_runtime_manager())
            .deployment_updater(StubDeploymentUpdater)
            .queue_client(StubBuildQueueClient)
            .build()
    }

    fn get_queue(name: &str) -> Queued {
        let enc = GzEncoder::new(Vec::new(), Compression::fast());
        let mut tar = tar::Builder::new(enc);

        for dir_entry in read_dir(format!("tests/deploy_layer/{name}")).unwrap() {
            let dir_entry = dir_entry.unwrap();
            if dir_entry.file_name() != "target" {
                let path = format!("{name}/{}", dir_entry.file_name().to_str().unwrap());

                if dir_entry.file_type().unwrap().is_dir() {
                    tar.append_dir_all(path, dir_entry.path()).unwrap();
                } else {
                    tar.append_path_with_name(dir_entry.path(), path).unwrap();
                }
            }
        }

        let enc = tar.into_inner().unwrap();
        let bytes = enc.finish().unwrap();

        println!("{name}: finished getting archive for test");

        Queued {
            id: Uuid::new_v4(),
            service_name: format!("deploy-layer-{name}"),
            service_id: Uuid::new_v4(),
            data: bytes,
            will_run_tests: false,
            tracing_context: Default::default(),
        }
    }
}

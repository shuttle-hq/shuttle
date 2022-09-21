use std::{collections::HashMap, env, str::FromStr};

use chrono::{DateTime, Utc};
use serde_json::json;
use shuttle_common::{DeploymentId, LogItem};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{field::Visit, metadata::ParseLevelError, Level, Subscriber};
use tracing_subscriber::Layer;

#[derive(Debug)]
pub struct Log {
    pub deployment_id: DeploymentId,
    pub datetime: DateTime<Utc>,
    pub item: LogItem,
}

pub struct Logger {
    deployment_id: DeploymentId,
    tx: UnboundedSender<Log>,
    filter: HashMap<String, Level>,
}

impl Logger {
    pub fn new(tx: UnboundedSender<Log>, deployment_id: DeploymentId) -> Self {
        let filter = if let Ok(rust_log) = env::var("RUST_LOG") {
            let rust_log = rust_log
                .split(',')
                .map(|item| {
                    // Try to get target and level if both are set
                    if let Some((target, level)) = item.split_once('=') {
                        Result::<(String, Level), ParseLevelError>::Ok((
                            target.to_string(),
                            Level::from_str(level)?,
                        ))
                    } else {
                        // Ok only target or level is set, but which is it
                        if let Ok(level) = Level::from_str(item) {
                            Ok((String::new(), level))
                        } else {
                            Ok((item.to_string(), Level::TRACE))
                        }
                    }
                })
                .filter_map(Result::ok);

            HashMap::from_iter(rust_log)
        } else {
            HashMap::from([(String::new(), Level::ERROR)])
        };

        Self {
            tx,
            deployment_id,
            filter,
        }
    }
}

impl<S> Layer<S> for Logger
where
    S: Subscriber,
{
    fn enabled(
        &self,
        metadata: &tracing::Metadata<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        metadata.level() <= &Level::INFO
    }

    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let datetime = Utc::now();
        let metadata = event.metadata();

        let item = {
            let mut visitor = JsonVisitor::default();
            event.record(&mut visitor);

            // drop log metadata as it is included in the other LogItem fields (target, file, line...)
            let fields: serde_json::Map<String, serde_json::Value> = visitor.0
                .into_iter()
                .filter(|(field, _)| !field.starts_with("log."))
                .collect();

            LogItem {
                level: metadata.level().to_string(),
                file: metadata.file().map(str::to_string),
                line: metadata.line(),
                target: metadata.target().to_string(),
                fields: serde_json::to_vec(&fields).unwrap(),
            }
        };

        self.tx
            .send(Log {
                item,
                datetime,
                deployment_id: self.deployment_id,
            })
            .expect("sending log should succeed");
    }
}


// Boilerplate for extracting the fields from the event
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

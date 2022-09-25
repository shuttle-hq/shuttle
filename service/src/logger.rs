use chrono::{DateTime, Utc};
use serde_json::json;
use shuttle_common::{DeploymentId, LogItem};
use tracing::{field::Visit, Subscriber};
use tracing_subscriber::Layer;

#[derive(Debug)]
pub struct Log {
    pub deployment_id: DeploymentId,
    pub datetime: DateTime<Utc>,
    pub item: LogItem,
}

pub struct Logger {
    deployment_id: DeploymentId,
    tx: crossbeam_channel::Sender<Log>,
}

impl Logger {
    pub fn new(tx: crossbeam_channel::Sender<Log>, deployment_id: DeploymentId) -> Self {
        Self { tx, deployment_id }
    }
}

impl<S> Layer<S> for Logger
where
    S: Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let datetime = Utc::now();

        let item = {
            let metadata = event.metadata();
            let mut visitor = JsonVisitor::default();
            event.record(&mut visitor);
            
            LogItem {
                level: metadata.level().to_string(),
                file: metadata.file().map(str::to_string),
                line: metadata.line(),
                target: metadata.target().to_string(),
                fields: serde_json::to_vec(&visitor.0).unwrap(),
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

impl JsonVisitor {
    /// Ignores log metadata as it is included in the other LogItem fields (target, file, line...)
    fn filter_insert(&mut self, field: &tracing::field::Field, value: serde_json::Value) {
        if !field.name().starts_with("log.") {
            self.0.insert(field.name().to_string(), json!(value));
        }
    }
}
impl Visit for JsonVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.filter_insert(field, json!(value));
    }
    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.filter_insert(field, json!(value));
    }
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.filter_insert(field, json!(value));
    }
    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.filter_insert(field, json!(value));
    }
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.filter_insert(field, json!(value));
    }
    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.filter_insert(field, json!(value.to_string()));
    }
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.filter_insert(field, json!(format!("{value:?}")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tracing_subscriber::prelude::*;

    #[test]
    fn logging() {
        let (s, r) = crossbeam_channel::unbounded();

        let logger = Logger::new(s, Default::default());

        tracing_subscriber::registry().with(logger).init();

        tracing::debug!("this is");
        tracing::info!("hi");
        tracing::warn!("from");
        tracing::error!("logger");

        let logs = r
            .try_iter()
            .map(|log| {
                let fields: serde_json::Map<String, serde_json::Value> =
                    serde_json::from_slice(&log.item.fields).unwrap();

                fields["message"].as_str().unwrap().to_owned()
            })
            .collect::<Vec<_>>();

        assert_eq!(logs, vec!["this is", "hi", "from", "logger",]);
    }
}

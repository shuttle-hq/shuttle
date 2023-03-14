use chrono::Utc;
use shuttle_common::{deployment::State, tracing::JsonVisitor, DeploymentId, LogItem};
use tokio::sync::mpsc::UnboundedSender;
use tracing::Subscriber;
use tracing_subscriber::Layer;

pub struct Logger {
    deployment_id: DeploymentId,
    tx: UnboundedSender<LogItem>,
}

impl Logger {
    pub fn new(tx: UnboundedSender<LogItem>, deployment_id: DeploymentId) -> Self {
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
                id: self.deployment_id,
                state: State::Running,
                level: metadata.level().into(),
                timestamp: datetime,
                file: visitor.file.or_else(|| metadata.file().map(str::to_string)),
                line: visitor.line.or_else(|| metadata.line()),
                target: visitor
                    .target
                    .unwrap_or_else(|| metadata.target().to_string()),
                fields: serde_json::to_vec(&visitor.fields).unwrap(),
            }
        };

        self.tx.send(item).expect("sending log should succeed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use shuttle_common::log::Level;
    use tokio::sync::mpsc;
    use tracing_subscriber::prelude::*;

    #[test]
    fn logging() {
        let (s, mut r) = mpsc::unbounded_channel();

        let logger = Logger::new(s, Default::default());

        let _guard = tracing_subscriber::registry().with(logger).set_default();

        tracing::debug!("this is");
        tracing::info!("hi");
        tracing::warn!("from");
        tracing::error!("logger");

        assert_eq!(
            r.blocking_recv().map(to_tuple),
            Some(("this is".to_string(), Level::Debug))
        );
        assert_eq!(
            r.blocking_recv().map(to_tuple),
            Some(("hi".to_string(), Level::Info))
        );
        assert_eq!(
            r.blocking_recv().map(to_tuple),
            Some(("from".to_string(), Level::Warn))
        );
        assert_eq!(
            r.blocking_recv().map(to_tuple),
            Some(("logger".to_string(), Level::Error))
        );
    }

    fn to_tuple(log: LogItem) -> (String, Level) {
        let fields: serde_json::Map<String, serde_json::Value> =
            serde_json::from_slice(&log.fields).unwrap();

        let message = fields["message"].as_str().unwrap().to_owned();

        (message, log.level)
    }
}

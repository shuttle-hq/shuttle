use std::time::SystemTime;

use chrono::Utc;
use prost_types::Timestamp;
use shuttle_common::tracing::JsonVisitor;
use shuttle_proto::runtime::{LogItem, LogLevel};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{
    span::{Attributes, Id},
    Level, Subscriber,
};
use tracing_subscriber::Layer;

pub struct Logger {
    tx: UnboundedSender<LogItem>,
}

impl Logger {
    pub fn new(tx: UnboundedSender<LogItem>) -> Self {
        Self { tx }
    }
}

impl<S> Layer<S> for Logger
where
    S: Subscriber,
{
    fn on_new_span(
        &self,
        attrs: &Attributes,
        _id: &Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let datetime = Utc::now();

        let item = {
            let metadata = attrs.metadata();
            let level = metadata.level();

            // Ignore span logs from the default level for #[instrument] (INFO) and below (greater than).
            // TODO: make this configurable
            if level >= &Level::INFO {
                return;
            }

            let mut visitor = JsonVisitor::default();
            attrs.record(&mut visitor);

            // Make the span name the log message
            visitor.fields.insert(
                "message".to_string(),
                format!("[span] {}", metadata.name()).into(),
            );

            LogItem {
                level: LogLevel::from(level) as i32,
                timestamp: Some(Timestamp::from(SystemTime::from(datetime))),
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
                level: LogLevel::from(metadata.level()) as i32,
                timestamp: Some(Timestamp::from(SystemTime::from(datetime))),
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

    use tokio::sync::mpsc;
    use tracing_subscriber::prelude::*;

    #[test]
    fn logging() {
        let (s, mut r) = mpsc::unbounded_channel();

        let logger = Logger::new(s);

        let _guard = tracing_subscriber::registry().with(logger).set_default();

        let span = tracing::info_span!("this is an info span");
        span.in_scope(|| {
            tracing::debug!("this is");
            tracing::info!("hi");
        });
        let span = tracing::warn_span!("this is a warn span");
        span.in_scope(|| {
            tracing::warn!("from");
            tracing::error!("logger");
        });

        assert_eq!(
            r.blocking_recv().map(to_tuple),
            Some(("this is".to_string(), LogLevel::Debug as i32))
        );
        assert_eq!(
            r.blocking_recv().map(to_tuple),
            Some(("hi".to_string(), LogLevel::Info as i32))
        );
        assert_eq!(
            r.blocking_recv().map(to_tuple),
            Some((
                "[span] this is a warn span".to_string(),
                LogLevel::Warn as i32
            ))
        );
        assert_eq!(
            r.blocking_recv().map(to_tuple),
            Some(("from".to_string(), LogLevel::Warn as i32))
        );
        assert_eq!(
            r.blocking_recv().map(to_tuple),
            Some(("logger".to_string(), LogLevel::Error as i32))
        );
    }

    fn to_tuple(log: LogItem) -> (String, i32) {
        let fields: serde_json::Map<String, serde_json::Value> =
            serde_json::from_slice(&log.fields).unwrap();

        let message = fields["message"].as_str().unwrap().to_owned();

        (message, log.level)
    }
}

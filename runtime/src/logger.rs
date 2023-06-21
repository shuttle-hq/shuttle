use shuttle_common::tracing::JsonVisitor;
use tracing::{
    span::{Attributes, Id},
    Level, Metadata, Subscriber,
};
use tracing_subscriber::Layer;

/// Record a single log
pub trait LogRecorder {
    fn record_log(&self, visitor: JsonVisitor, metadata: &Metadata);
}

pub struct OtlpRecorder;

impl LogRecorder for OtlpRecorder {
    fn record_log(&self, visitor: JsonVisitor, metadata: &Metadata) {
        todo!()
    }
}

pub struct Logger<R> {
    recorder: R,
}

impl<R> Logger<R> {
    pub fn new(recorder: R) -> Self {
        Self { recorder }
    }
}

impl<S, R> Layer<S> for Logger<R>
where
    S: Subscriber,
    R: LogRecorder + Send + Sync + 'static,
{
    fn on_new_span(
        &self,
        attrs: &Attributes,
        _id: &Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
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

        self.recorder.record_log(visitor, metadata);
    }

    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = JsonVisitor::default();

        event.record(&mut visitor);
        let metadata = event.metadata();

        self.recorder.record_log(visitor, metadata);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    use super::*;

    use tracing_subscriber::prelude::*;

    #[derive(Default, Clone)]
    struct DummyRecorder {
        lines: Arc<Mutex<VecDeque<(Level, String)>>>,
    }

    impl LogRecorder for DummyRecorder {
        fn record_log(&self, visitor: JsonVisitor, metadata: &Metadata) {
            self.lines.lock().unwrap().push_back((
                metadata.level().clone(),
                visitor
                    .fields
                    .get("message")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string(),
            ));
        }
    }

    #[test]
    fn logging() {
        let recorder = DummyRecorder::default();
        let logger = Logger::new(recorder.clone());

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
            recorder.lines.lock().unwrap().pop_front(),
            Some((Level::DEBUG, "this is".to_string()))
        );
        assert_eq!(
            recorder.lines.lock().unwrap().pop_front(),
            Some((Level::INFO, "hi".to_string()))
        );
        assert_eq!(
            recorder.lines.lock().unwrap().pop_front(),
            Some((Level::WARN, "[span] this is a warn span".to_string()))
        );
        assert_eq!(
            recorder.lines.lock().unwrap().pop_front(),
            Some((Level::WARN, "from".to_string()))
        );
        assert_eq!(
            recorder.lines.lock().unwrap().pop_front(),
            Some((Level::ERROR, "logger".to_string()))
        );
    }
}

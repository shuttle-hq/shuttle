use std::{collections::HashMap, env, str::FromStr};

use chrono::{DateTime, Utc};
use shuttle_common::{DeploymentId, LogItem};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{Level, Subscriber, metadata::ParseLevelError};
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

        let item = LogItem {
            body: format!("{:?}", event.fields()),
            level: metadata.level().to_string(),
            target: metadata.target().to_string(),
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
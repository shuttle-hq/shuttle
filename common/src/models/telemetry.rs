use serde::{Deserialize, Serialize};

/// Status of a telemetry export configuration for an external sink
#[derive(Eq, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[typeshare::typeshare]
pub struct TelemetrySinkStatus {
    /// Indicates that the associated project is configured to export telemetry data to this sink
    enabled: bool,
}

/// A safe-for-display representation of the current telemetry export configuration for a given project
#[derive(Eq, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[typeshare::typeshare]
pub struct ProjectTelemetryConfigResponse {
    betterstack: Option<TelemetrySinkStatus>,
    datadog: Option<TelemetrySinkStatus>,
    grafana_cloud: Option<TelemetrySinkStatus>,
}

impl From<Vec<ProjectTelemetrySinkConfig>> for ProjectTelemetryConfigResponse {
    fn from(value: Vec<ProjectTelemetrySinkConfig>) -> Self {
        let mut instance = Self::default();

        for sink in value {
            match sink {
                ProjectTelemetrySinkConfig::Betterstack { .. } => {
                    instance.betterstack = Some(TelemetrySinkStatus { enabled: true })
                }
                ProjectTelemetrySinkConfig::Datadog { .. } => {
                    instance.datadog = Some(TelemetrySinkStatus { enabled: true })
                }
                ProjectTelemetrySinkConfig::GrafanaCloud { .. } => {
                    instance.grafana_cloud = Some(TelemetrySinkStatus { enabled: true })
                }
            }
        }

        instance
    }
}

/// The user-supplied config required to export telemetry to a given external sink
#[derive(Eq, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "content")]
#[typeshare::typeshare]
pub enum ProjectTelemetrySinkConfig {
    /// [Betterstack](https://betterstack.com/docs/logs/open-telemetry/)
    Betterstack(BetterstackConfig),
    /// [Datadog](https://docs.datadoghq.com/opentelemetry/collector_exporter/otel_collector_datadog_exporter)
    Datadog(DatadogConfig),
    /// [Grafana Cloud](https://grafana.com/docs/grafana-cloud/send-data/otlp/)
    GrafanaCloud(GrafanaCloudConfig),
}

#[derive(Eq, Clone, PartialEq, Serialize, Deserialize)]
#[typeshare::typeshare]
pub struct BetterstackConfig {
    source_token: String,
}
#[derive(Eq, Clone, PartialEq, Serialize, Deserialize)]
#[typeshare::typeshare]
pub struct DatadogConfig {
    api_key: String,
}
#[derive(Eq, Clone, PartialEq, Serialize, Deserialize)]
#[typeshare::typeshare]
pub struct GrafanaCloudConfig {
    token: String,
    endpoint: String,
    instance_id: String,
}

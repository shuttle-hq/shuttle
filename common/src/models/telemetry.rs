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
pub struct TelemetryConfigResponse {
    betterstack: Option<TelemetrySinkStatus>,
    datadog: Option<TelemetrySinkStatus>,
    grafana_cloud: Option<TelemetrySinkStatus>,
}

impl From<Vec<TelemetrySinkConfig>> for TelemetryConfigResponse {
    fn from(value: Vec<TelemetrySinkConfig>) -> Self {
        let mut instance = Self::default();

        for sink in value {
            match sink {
                TelemetrySinkConfig::Betterstack(_) => {
                    instance.betterstack = Some(TelemetrySinkStatus { enabled: true })
                }
                TelemetrySinkConfig::Datadog(_) => {
                    instance.datadog = Some(TelemetrySinkStatus { enabled: true })
                }
                TelemetrySinkConfig::GrafanaCloud(_) => {
                    instance.grafana_cloud = Some(TelemetrySinkStatus { enabled: true })
                }
            }
        }

        instance
    }
}

/// The user-supplied config required to export telemetry to a given external sink
#[derive(
    // std
    Eq,
    Clone,
    PartialEq,
    // serde
    Serialize,
    Deserialize,
    // strum
    strum::AsRefStr,
    strum::EnumDiscriminants,
)]
#[cfg_attr(feature = "integration-tests", derive(Debug))]
#[serde(tag = "type", content = "content", rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[typeshare::typeshare]
#[strum_discriminants(derive(Serialize, Deserialize, strum::AsRefStr))]
#[strum_discriminants(serde(rename_all = "snake_case"))]
#[strum_discriminants(strum(serialize_all = "snake_case"))]
pub enum TelemetrySinkConfig {
    /// [Betterstack](https://betterstack.com/docs/logs/open-telemetry/)
    Betterstack(BetterstackConfig),
    /// [Datadog](https://docs.datadoghq.com/opentelemetry/collector_exporter/otel_collector_datadog_exporter)
    Datadog(DatadogConfig),
    /// [Grafana Cloud](https://grafana.com/docs/grafana-cloud/send-data/otlp/)
    GrafanaCloud(GrafanaCloudConfig),
}

impl TelemetrySinkConfig {
    pub fn as_db_type(&self) -> String {
        format!("project::telemetry::{}::config", self.as_ref())
    }
}

impl TelemetrySinkConfigDiscriminants {
    pub fn as_db_type(&self) -> String {
        format!("project::telemetry::{}::config", self.as_ref())
    }
}

#[derive(Eq, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "integration-tests", derive(Debug))]
#[typeshare::typeshare]
pub struct BetterstackConfig {
    #[serde(default = "default_betterstack_host")]
    pub ingesting_host: String,
    pub source_token: String,
}
fn default_betterstack_host() -> String {
    "in-otel.logs.betterstack.com".to_owned()
}
#[derive(Eq, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "integration-tests", derive(Debug))]
#[typeshare::typeshare]
pub struct DatadogConfig {
    pub api_key: String,
}
#[derive(Eq, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "integration-tests", derive(Debug))]
#[typeshare::typeshare]
pub struct GrafanaCloudConfig {
    pub token: String,
    pub endpoint: String,
    pub instance_id: String,
}

#[cfg(feature = "integration-tests")]
impl From<BetterstackConfig> for TelemetrySinkConfig {
    fn from(value: BetterstackConfig) -> Self {
        TelemetrySinkConfig::Betterstack(value)
    }
}

#[cfg(feature = "integration-tests")]
impl From<DatadogConfig> for TelemetrySinkConfig {
    fn from(value: DatadogConfig) -> Self {
        TelemetrySinkConfig::Datadog(value)
    }
}

#[cfg(feature = "integration-tests")]
impl From<GrafanaCloudConfig> for TelemetrySinkConfig {
    fn from(value: GrafanaCloudConfig) -> Self {
        TelemetrySinkConfig::GrafanaCloud(value)
    }
}

#[cfg(feature = "integration-tests")]
impl std::str::FromStr for TelemetrySinkConfig {
    type Err = serde_json::Error;

    fn from_str(config: &str) -> Result<Self, Self::Err> {
        serde_json::from_str::<BetterstackConfig>(config)
            .map(Self::from)
            .inspect_err(|error| {
                tracing::debug!(
                    %config,
                    %error,
                    "cannot deserialize config as valid Betterstack configuration",
                )
            })
            .or(serde_json::from_str::<DatadogConfig>(config)
                .map(Self::from)
                .inspect_err(|error| {
                    tracing::debug!(
                        %config,
                        %error,
                        "cannot deserialize config as valid DataDog configuration",
                    )
                }))
            .or(serde_json::from_str::<GrafanaCloudConfig>(config)
                .map(Self::from)
                .inspect_err(|error| {
                    tracing::debug!(
                        %config,
                        %error,
                        "cannot deserialize config as valid GrafanaCloud configuration",
                    )
                }))
            .map_err(|_| {
                <serde_json::Error as serde::de::Error>::custom(format!(
                    "configuration does not match any known external telemetry sink: {}",
                    config
                ))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sink_config_enum() {
        assert_eq!(
            "betterstack",
            TelemetrySinkConfig::Betterstack(BetterstackConfig {
                ingesting_host: "".into(),
                source_token: "".into()
            })
            .as_ref()
        );
        assert_eq!(
            "project::telemetry::betterstack::config",
            TelemetrySinkConfig::Betterstack(BetterstackConfig {
                ingesting_host: "".into(),
                source_token: "".into()
            })
            .as_db_type()
        );

        assert_eq!(
            "betterstack",
            TelemetrySinkConfigDiscriminants::Betterstack.as_ref()
        );
        assert_eq!(
            "grafana_cloud",
            TelemetrySinkConfigDiscriminants::GrafanaCloud.as_ref()
        );
        assert_eq!(
            "\"betterstack\"",
            serde_json::to_string(&TelemetrySinkConfigDiscriminants::Betterstack).unwrap()
        );
        assert_eq!(
            "\"grafana_cloud\"",
            serde_json::to_string(&TelemetrySinkConfigDiscriminants::GrafanaCloud).unwrap()
        );
    }
}

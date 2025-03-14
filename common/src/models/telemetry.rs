use std::borrow::Cow;

use serde::{Deserialize, Serialize};

const fn default_betterstack_host() -> Cow<'static, str> {
    Cow::Borrowed("in-otel.logs.betterstack.com")
}

/// Status of a telemetry export configuration for an external sink
#[derive(Eq, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct TelemetrySinkStatus {
    /// Indicates that the associated project is configured to export telemetry data to this sink
    enabled: bool,
}

/// A safe-for-display representation of the current telemetry export configuration for a given project
#[derive(Eq, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
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
                TelemetrySinkConfig::Debug(_) => {}
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
#[cfg_attr(any(test, feature = "integration-tests"), derive(strum::EnumIter))]
#[serde(tag = "type", content = "content", rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[cfg_attr(
    any(test, feature = "integration-tests"),
    strum_discriminants(derive(strum::EnumIter))
)]
#[strum_discriminants(derive(Serialize, Deserialize, strum::AsRefStr))]
#[strum_discriminants(serde(rename_all = "snake_case"))]
#[strum_discriminants(strum(serialize_all = "snake_case"))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub enum TelemetrySinkConfig {
    /// [Betterstack](https://betterstack.com/docs/logs/open-telemetry/)
    Betterstack(BetterstackConfig),

    /// [Datadog](https://docs.datadoghq.com/opentelemetry/collector_exporter/otel_collector_datadog_exporter)
    Datadog(DatadogConfig),

    /// [Grafana Cloud](https://grafana.com/docs/grafana-cloud/send-data/otlp/)
    GrafanaCloud(GrafanaCloudConfig),

    /// Internal Debugging
    #[doc(hidden)]
    #[typeshare(skip)]
    #[strum_discriminants(doc(hidden))]
    Debug(serde_json::Value),
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
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct BetterstackConfig {
    #[serde(default = "default_betterstack_host")]
    pub ingesting_host: Cow<'static, str>,
    pub source_token: String,
}

#[cfg(any(test, feature = "integration-tests"))]
impl Default for BetterstackConfig {
    fn default() -> Self {
        Self {
            source_token: "some-source-token".into(),
            ingesting_host: default_betterstack_host(),
        }
    }
}

#[derive(Eq, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "integration-tests", derive(Debug))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct DatadogConfig {
    pub api_key: String,
}

#[cfg(any(test, feature = "integration-tests"))]
impl Default for DatadogConfig {
    fn default() -> Self {
        Self {
            api_key: "some-api-key".into(),
        }
    }
}

#[derive(Eq, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "integration-tests", derive(Debug))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct GrafanaCloudConfig {
    pub token: String,
    pub endpoint: String,
    pub instance_id: String,
}

#[cfg(any(test, feature = "integration-tests"))]
impl Default for GrafanaCloudConfig {
    fn default() -> Self {
        Self {
            token: "some-auth-token".into(),
            instance_id: String::from("0000000"),
            endpoint: "https://prometheus-env-id-env-region.grafana.net/api/prom/push".into(),
        }
    }
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
        for variant in <TelemetrySinkConfig as strum::IntoEnumIterator>::iter() {
            match variant {
                sink @ TelemetrySinkConfig::Betterstack(_) => {
                    assert_eq!("betterstack", sink.as_ref());
                    assert_eq!("project::telemetry::betterstack::config", sink.as_db_type());
                }
                sink @ TelemetrySinkConfig::Datadog(_) => {
                    assert_eq!("datadog", sink.as_ref());
                    assert_eq!("project::telemetry::datadog::config", sink.as_db_type());
                }
                sink @ TelemetrySinkConfig::GrafanaCloud(_) => {
                    assert_eq!("grafana_cloud", sink.as_ref());
                    assert_eq!(
                        "project::telemetry::grafana_cloud::config",
                        sink.as_db_type()
                    );
                }
                sink @ TelemetrySinkConfig::Debug(_) => {
                    assert_eq!("debug", sink.as_ref());
                    assert_eq!("project::telemetry::debug::config", sink.as_db_type());
                }
            }
        }

        for variant in <TelemetrySinkConfigDiscriminants as strum::IntoEnumIterator>::iter() {
            match variant {
                discriminant @ TelemetrySinkConfigDiscriminants::Betterstack => {
                    assert_eq!("betterstack", discriminant.as_ref());
                    assert_eq!(
                        r#""betterstack""#,
                        serde_json::to_string(&discriminant).unwrap()
                    );
                }
                discriminant @ TelemetrySinkConfigDiscriminants::Datadog => {
                    assert_eq!("datadog", discriminant.as_ref());
                    assert_eq!(
                        r#""datadog""#,
                        serde_json::to_string(&discriminant).unwrap()
                    );
                }
                discriminant @ TelemetrySinkConfigDiscriminants::GrafanaCloud => {
                    assert_eq!("grafana_cloud", discriminant.as_ref());
                    assert_eq!(
                        r#""grafana_cloud""#,
                        serde_json::to_string(&discriminant).unwrap()
                    );
                }
                discriminant @ TelemetrySinkConfigDiscriminants::Debug => {
                    assert_eq!("debug", discriminant.as_ref());
                    assert_eq!(r#""debug""#, serde_json::to_string(&discriminant).unwrap());
                }
            }
        }
    }
}

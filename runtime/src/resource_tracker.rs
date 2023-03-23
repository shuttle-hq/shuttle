use anyhow::Context;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use shuttle_service::ResourceBuilder;

use crate::ProvisionerFactory;

/// Used to keep track of which resources have been provisioned in the past and what is being provisioned for this deployment
pub struct ResourceTracker;

impl ResourceTracker {
    /// Get the output of a resource that has been constructed in the past if it exists
    pub fn get_cached_output(&self, namespace: &str, config: &Value) -> Option<Value> {
        let value = serde_json::json!({"address_private":"localhost","address_public":"localhost","database_name":"postgres","engine":"postgres","port":"21673","role_name":"postgres","role_password":"postgres"});

        Some(value)
    }

    /// Record a resource that has been requested
    pub fn record_resource(&mut self, namespace: &str, config: Value, output: Value) {
        println!("config: {}", config);
        println!("output: {}", output);
    }
}

/// Helper function to get a resource from a builder.
///
/// This function is called by the codegen to create each type of needed resource.
pub async fn get_resource<B, T, O>(
    builder: B,
    namespace: &str,
    factory: &mut ProvisionerFactory,
    resource_tracker: &mut ResourceTracker,
) -> Result<T, shuttle_service::Error>
where
    B: ResourceBuilder<T, Output = O>,
    O: Serialize + DeserializeOwned,
{
    let config =
        serde_json::to_value(&builder).context("failed to turn builder config into a value")?;
    let output = if let Some(output) = resource_tracker.get_cached_output(namespace, &config) {
        match serde_json::from_value(output) {
            Ok(output) => output,
            Err(err) => {
                tracing::warn!(
                    error = &err as &dyn std::error::Error,
                    "failed to get output from past value. Will build a new output instead"
                );

                builder
                    .output(factory)
                    .await
                    .context("failed to provision resource again")?
            }
        }
    } else {
        builder
            .output(factory)
            .await
            .context("failed to provision resource")?
    };

    let resource = B::build(&output).await?;

    let output =
        serde_json::to_value(&output).context("failed to turn builder output into a value")?;

    resource_tracker.record_resource(namespace, config, output);

    Ok(resource)
}

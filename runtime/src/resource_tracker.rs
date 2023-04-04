use std::sync::{Arc, Mutex};

use anyhow::Context;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use shuttle_common::resource::{self, Type};
use shuttle_service::ResourceBuilder;

use crate::ProvisionerFactory;

/// Used to keep track of which resources have been provisioned in the past and what is being provisioned for this deployment
pub struct ResourceTracker {
    past_resources: Vec<resource::Response>,
    new_resources: Arc<Mutex<Vec<resource::Response>>>,
}

impl ResourceTracker {
    pub fn new(
        past_resources: Vec<resource::Response>,
        new_resources: Arc<Mutex<Vec<resource::Response>>>,
    ) -> Self {
        Self {
            past_resources,
            new_resources,
        }
    }

    /// Get the output of a resource that has been constructed in the past if it exists
    pub fn get_cached_output(&self, r#type: Type, config: &Value) -> Option<Value> {
        self.past_resources
            .iter()
            .find(|resource| resource.r#type == r#type && resource.config == *config)
            .map(|resource| resource.data.clone())
    }

    /// Record a resource that has been requested
    pub fn record_resource(&mut self, r#type: Type, config: Value, output: Value) {
        self.new_resources
            .lock()
            .expect("to get lock on new resources")
            .push(resource::Response {
                r#type,
                config,
                data: output,
            })
    }
}

/// Helper function to get a resource from a builder.
///
/// This function is called by the codegen to create each type of needed resource.
pub async fn get_resource<B, T, O>(
    builder: B,
    factory: &mut ProvisionerFactory,
    resource_tracker: &mut ResourceTracker,
) -> Result<T, shuttle_service::Error>
where
    B: ResourceBuilder<T, Output = O>,
    O: Serialize + DeserializeOwned,
{
    let config = serde_json::to_value(builder.config())
        .context("failed to turn builder config into a value")?;
    let output = if let Some(output) = resource_tracker.get_cached_output(B::TYPE, &config) {
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

    resource_tracker.record_resource(B::TYPE, config, output);

    Ok(resource)
}

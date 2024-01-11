use std::sync::{Arc, Mutex};

use anyhow::Context;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use shuttle_common::resource::{self, Type};
use shuttle_service::{IntoResource, ResourceBuilder};

use crate::__internals::ProvisionerFactory;

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
        // Secrets are returning unit configs, which deserialised come as a `serde_json::Value::Null`.
        // We always return the cached output for them, even if they change from a previous deployment.
        // We have to always call `output()` on them to get the latest secrets, since we don't track a
        // config for them that can change if secrets changed.
        if config.is_null() {
            return None;
        }

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

macro_rules! log {
    ($msg:expr) => {
        println!("[Resource][{}] {}", B::TYPE, $msg);
    };
}

/// Helper function to get a resource from a builder.
///
/// This function is called by the loader (see codegen) to create each type of needed resource.
pub async fn get_resource<B, O, T>(
    builder: B,
    factory: &mut ProvisionerFactory,
    resource_tracker: &mut ResourceTracker,
) -> Result<T, shuttle_service::Error>
where
    B: ResourceBuilder<Output = O>,
    O: Serialize + DeserializeOwned + IntoResource<Output = T>,
{
    log!("Getting resource");

    let config = serde_json::to_value(builder.config())
        .context("failed to turn builder config into a value")?;

    log!(format!("Using config: {}", config)); // TODO: This can contain secrets

    let output = resource_tracker.get_cached_output(B::TYPE, &config)
        .and_then(|output| {
            log!("Found past output for this config");
            match serde_json::from_value(output) {
                Ok(output) => Some(output),
                Err(err) => {
                    log!(format!(
                        "Failed to get output from past config ({err}). Will build a new output instead."
                    ));
                    None
                }
            }
        })
        ;
    let output = match output {
        Some(output) => output,
        None => {
            log!("Provisioning. This can take a while...");

            let output = builder
                .output(factory)
                .await
                .context("failed to provision resource")?;

            log!("Done provisioning");

            output
        }
    };

    let output_value =
        serde_json::to_value(&output).context("failed to turn builder output into a JSON value")?;

    log!("Connecting resource");
    let resource: T = output.init().await?;
    log!("Resource connected");

    resource_tracker.record_resource(B::TYPE, config, output_value);

    Ok(resource)
}

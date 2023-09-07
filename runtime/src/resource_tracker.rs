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
        // Secrets are returning unit configs, which deserialised come as a serde_json::Value::Null`.
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
    log!("Getting resource");

    let config = serde_json::to_value(builder.config())
        .context("failed to turn builder config into a value")?;

    log!(format!("Using config: {}", config));

    let output = if let Some(output) = resource_tracker.get_cached_output(B::TYPE, &config) {
        log!("Found past output from config");

        match serde_json::from_value(output) {
            Ok(output) => output,
            Err(err) => {
                log!(format!(
                    "failed to get output from past value ({err}). Will build a new output instead"
                ));

                log!("Provisioning. This can take a while...");

                let output = builder
                    .output(factory)
                    .await
                    .context("failed to provision resource again")?;

                log!("Done provisioning");

                output
            }
        }
    } else {
        log!("Past output for config does not exist");

        log!("Provisioning. This can take a while...");

        let output = builder
            .output(factory)
            .await
            .context("failed to provision resource")?;

        log!("Done provisioning");

        output
    };

    log!("Connecting resource");
    let resource = B::build(&output).await?;
    log!("Resource connected");

    let output =
        serde_json::to_value(&output).context("failed to turn builder output into a value")?;

    resource_tracker.record_resource(B::TYPE, config, output);

    Ok(resource)
}

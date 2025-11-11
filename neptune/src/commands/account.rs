use anyhow::{bail, Context, Result};
use cargo_shuttle::args::OutputMode;
use crossterm::style::Stylize;
use dialoguer::{theme::ColorfulTheme, Password};
use http::HeaderMap;
use shuttle_api_client::ShuttleApiClient;
use shuttle_common::constants::headers::X_CARGO_SHUTTLE_VERSION;
use std::collections::HashMap;

use crate::{
    args::{LoginArgs, LogoutArgs},
    ui::AiUi,
    Neptune, NeptuneCommandOutput,
};

impl Neptune {
    pub async fn login(&mut self, login_args: LoginArgs) -> Result<NeptuneCommandOutput> {
        // Persona UI (after prompt to avoid cluttering the password input)
        let ui = AiUi::new(&self.config.config().output_mode, self.global_args.verbose);
        ui.header("Login");
        let api_key = match login_args.api_key {
            Some(api_key) => api_key,
            None => {
                // if login_args.prompt {
                Password::with_theme(&ColorfulTheme::default())
                    .with_prompt("API key")
                    .validate_with(|input: &String| {
                        if input.is_empty() {
                            return Err("Empty API key was provided");
                        }
                        Ok(())
                    })
                    .interact()?
                // } else {
                //     // device auth flow via Shuttle Console
                //     self.device_auth(login_args.console_url).await?
                // }
            }
        };

        ui.step("", "Validating API key");

        // Verify API key without persisting it yet
        let cfg = self.config.config();
        let headers = HeaderMap::try_from(&HashMap::from([(
            X_CARGO_SHUTTLE_VERSION.clone(),
            crate::VERSION.to_owned(),
        )]))
        .unwrap();
        let temp_client = ShuttleApiClient::new(
            cfg.api_url.clone(),
            Some(api_key.clone()),
            Some(headers),
            None,
        );
        let response = temp_client
            .get("/users/me", Option::<()>::None)
            .await
            .context("failed to check API key validity")?;
        let status = response.status();
        if status.as_u16() != 200 {
            // Read body for a helpful error, but don't fail if body can't be read
            let body = response.text().await.unwrap_or_default();
            if body.contains("unauthorized") {
                bail!("login failed - invalid API key");
            } else {
                bail!("login failed - {}", body);
            }
        }
        let raw_json = response
            .text()
            .await
            .context("failed to read user response")?;
        // Only now that the key has been validated, persist it and refresh the client
        self.config
            .modify_global(|g| g.api_key = Some(api_key.clone()))?;
        self.refresh_api_client()?;
        let user_id = serde_json::from_str::<serde_json::Value>(&raw_json)
            .ok()
            .and_then(|v| v.get("id").and_then(|s| s.as_str()).map(|s| s.to_string()));

        match self.config.config().output_mode {
            OutputMode::Normal => {
                if let Some(id) = user_id {
                    ui.success(format!("✅ Logged in as {}", id.bold()));
                } else {
                    ui.success("✅ Logged in");
                }
            }
            OutputMode::Json => {
                println!("{}", raw_json);
            }
        }

        Ok(NeptuneCommandOutput::None)
    }

    pub async fn logout(&mut self, _logout_args: LogoutArgs) -> Result<NeptuneCommandOutput> {
        // Reset API key endpoint:
        // if logout_args.reset_api_key {
        //     let client = self.client.as_ref().unwrap();
        //     client.reset_api_key().await.context("Resetting API key")?;
        //     eprintln!("Successfully reset the API key.");
        // }

        // Save global config and reload API client
        self.config.modify_global(|g| g.api_key = None)?;
        // TODO: clear the key from local configs too?
        self.refresh_api_client()?;

        eprintln!("Successfully logged out.");
        eprintln!(" -> Use `neptune login` to log in again.");

        Ok(NeptuneCommandOutput::None)
    }
}

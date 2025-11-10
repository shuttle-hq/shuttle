use anyhow::{Context, Result};
use cargo_shuttle::args::OutputMode;
use crossterm::style::Stylize;
use dialoguer::{theme::ColorfulTheme, Password};

use crate::{
    args::{LoginArgs, LogoutArgs},
    Neptune, NeptuneCommandOutput,
};

impl Neptune {
    pub async fn login(&mut self, login_args: LoginArgs) -> Result<NeptuneCommandOutput> {
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

        // Save global config and reload API client
        self.config
            .modify_global(|g| g.api_key = Some(api_key.clone()))?;
        self.refresh_api_client()?;

        // Verify API key using the API; be lenient with response schema
        let response = self
            .client
            .api_client
            .get("/users/me", Option::<()>::None)
            .await
            .context("failed to check API key validity")?;
        let raw_json = response
            .text()
            .await
            .context("failed to read user response")?;
        let user_id = serde_json::from_str::<serde_json::Value>(&raw_json)
            .ok()
            .and_then(|v| v.get("id").and_then(|s| s.as_str()).map(|s| s.to_string()));

        match self.config.config().output_mode {
            OutputMode::Normal => {
                if let Some(id) = user_id {
                    println!("Logged in as {}", id.bold());
                } else {
                    println!("Logged in.");
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

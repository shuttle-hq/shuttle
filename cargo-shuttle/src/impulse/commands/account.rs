use anyhow::{Context, Result};
use crossterm::style::Stylize;
use dialoguer::{theme::ColorfulTheme, Password};

use crate::{
    args::OutputMode,
    impulse::{
        args::{LoginArgs, LogoutArgs},
        Impulse, ImpulseCommandOutput,
    },
};

impl Impulse {
    pub async fn login(&mut self, login_args: LoginArgs) -> Result<ImpulseCommandOutput> {
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
        tracing::debug!("Saving global config");
        self.config
            .modify_global(|g| g.api_key = Some(api_key.clone()))?;
        self.client = Some(self.make_api_client()?);

        // if offline {
        //     eprintln!("INFO: Skipping API key verification");
        let (user, raw_json) = self
            .client
            .as_ref()
            .unwrap()
            // TODO: use actual impulse endpoint
            .inner
            .get_current_user()
            .await
            .context("failed to check API key validity")?
            .into_parts();

        match self.config.get_config()?.output_mode {
            OutputMode::Normal => {
                println!("Logged in as {}", user.id.bold());
            }
            OutputMode::Json => {
                println!("{}", raw_json);
            }
        }

        Ok(ImpulseCommandOutput::None)
    }

    pub async fn logout(&mut self, _logout_args: LogoutArgs) -> Result<ImpulseCommandOutput> {
        // if logout_args.reset_api_key {
        //     let client = self.client.as_ref().unwrap();
        //     client.reset_api_key().await.context("Resetting API key")?;
        //     eprintln!("Successfully reset the API key.");
        // }

        // Save global config and reload API client
        tracing::debug!("Saving global config");
        self.config.modify_global(|g| g.api_key = None)?;
        // TODO: clear the key from local configs too?
        self.client = Some(self.make_api_client()?);

        eprintln!("Successfully logged out.");
        eprintln!(" -> Use `impulse login` to log in again.");

        Ok(ImpulseCommandOutput::None)
    }
}

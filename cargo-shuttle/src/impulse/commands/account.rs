use anyhow::Result;
use dialoguer::{theme::ColorfulTheme, Password};
use shuttle_common::config::ConfigManager;

use crate::impulse::{
    args::{LoginArgs, LogoutArgs},
    config::ImpulseConfig,
    Impulse, ImpulseCommandOutput,
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
        let mut global = self
            .config
            .global
            .open::<ImpulseConfig>()
            .unwrap_or_default();
        global.api_key = Some(api_key);
        self.config.global.save(&global)?;
        self._client = Some(self.make_api_client());

        // TODO: validate successful login with API call

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
        if let Ok(mut global) = self.config.global.open::<ImpulseConfig>() {
            global.api_key = None;
            self.config.global.save(&global)?;
            self._client = Some(self.make_api_client());
        };
        // TODO: clear the key from local configs too?

        eprintln!("Successfully logged out.");
        eprintln!(" -> Use `impulse login` to log in again.");

        Ok(ImpulseCommandOutput::None)
    }
}

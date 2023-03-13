//! Shuttle service integration for the Serenity discord bot framework.
//! ## Example
//! ```rust,no_run
//! use anyhow::anyhow;
//! use serenity::async_trait;
//! use serenity::model::channel::Message;
//! use serenity::model::gateway::Ready;
//! use serenity::prelude::*;
//! use shuttle_secrets::SecretStore;
//! use tracing::{error, info};
//!
//! struct Bot;
//!
//! #[async_trait]
//! impl EventHandler for Bot {
//!     async fn message(&self, ctx: Context, msg: Message) {
//!         if msg.content == "!hello" {
//!             if let Err(e) = msg.channel_id.say(&ctx.http, "world!").await {
//!                 error!("Error sending message: {:?}", e);
//!             }
//!         }
//!     }
//!
//!     async fn ready(&self, _: Context, ready: Ready) {
//!         info!("{} is connected!", ready.user.name);
//!     }
//! }
//!
//! #[shuttle_runtime::main]
//! async fn serenity(
//!     #[shuttle_secrets::Secrets] secret_store: SecretStore,
//! ) -> shuttle_serenity::ShuttleSerenity {
//!     // Get the discord token set in `Secrets.toml`
//!     let token = if let Some(token) = secret_store.get("DISCORD_TOKEN") {
//!         token
//!     } else {
//!         return Err(anyhow!("'DISCORD_TOKEN' was not found").into());
//!     };
//!
//!     // Set gateway intents, which decides what events the bot will be notified about
//!     let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
//!
//!     let client = Client::builder(&token, intents)
//!         .event_handler(Bot)
//!         .await
//!         .expect("Err creating client");
//!
//!     Ok(client.into())
//! }
//!
//! ```
use shuttle_runtime::{CustomError, Error};
use std::net::SocketAddr;

/// A wrapper type for [serenity::Client] so we can implement [shuttle_runtime::Service] for it.
pub struct SerenityService(pub serenity::Client);

#[shuttle_runtime::async_trait]
impl shuttle_runtime::Service for SerenityService {
    /// Takes the client that is returned by the user in their [shuttle_runtime::main] function
    /// and starts it.
    async fn bind(mut self, _addr: SocketAddr) -> Result<(), Error> {
        self.0.start().await.map_err(CustomError::new)?;

        Ok(())
    }
}

impl From<serenity::Client> for SerenityService {
    fn from(router: serenity::Client) -> Self {
        Self(router)
    }
}

/// The return type that should be returned from the [shuttle_runtime::main] function.
pub type ShuttleSerenity = Result<SerenityService, Error>;

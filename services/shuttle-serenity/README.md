## Shuttle service integration for the Serenity discord bot framework.

### Example

```rust,no_run
use anyhow::anyhow;
#[cfg(feature = "serenity-0-12")]
use serenity_0_12::async_trait;
#[cfg(feature = "serenity-0-12")]
use serenity_0_12::model::channel::Message;
#[cfg(feature = "serenity-0-12")]
use serenity_0_12::model::gateway::Ready;
#[cfg(feature = "serenity-0-12")]
use serenity_0_12::prelude::*;
#[cfg(feature = "serenity")]
use serenity::async_trait;
#[cfg(feature = "serenity")]
use serenity::model::channel::Message;
#[cfg(feature = "serenity")]
use serenity::model::gateway::Ready;
#[cfg(feature = "serenity")]
use serenity::prelude::*;

use shuttle_secrets::SecretStore;
use tracing::{error, info};

struct Bot;

#[async_trait]
impl EventHandler for Bot {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "!hello" {
            if let Err(e) = msg.channel_id.say(&ctx.http, "world!").await {
                error!("Error sending message: {:?}", e);
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
    }
}

#[shuttle_runtime::main]
async fn serenity(
    #[shuttle_secrets::Secrets] secret_store: SecretStore,
) -> shuttle_serenity::ShuttleSerenity {
    // Get the discord token set in `Secrets.toml`
    let token = if let Some(token) = secret_store.get("DISCORD_TOKEN") {
        token
    } else {
        return Err(anyhow!("'DISCORD_TOKEN' was not found").into());
    };

    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    let client = Client::builder(&token, intents)
        .event_handler(Bot)
        .await
        .expect("Err creating client");

    Ok(client.into())
}
```

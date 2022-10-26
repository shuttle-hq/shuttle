use clap::Parser;
use serenity::prelude::*;
use shuttle_next::{args::Args, Bot};
use std::env;
use std::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    let _args = Args::parse();

    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    let token = env::var("DISCORD_TOKEN").unwrap();
    let src = env::var("BOT_SRC").unwrap();

    let mut client = Bot::new(src).into_client(token.as_str(), intents).await;
    client.start().await.unwrap();

    Ok(())
}

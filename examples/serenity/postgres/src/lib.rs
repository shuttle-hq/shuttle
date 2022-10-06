use anyhow::Context as _;
use serenity::async_trait;
use serenity::model::application::command::CommandOptionType;
use serenity::model::application::interaction::application_command::CommandDataOptionValue;
use serenity::model::application::interaction::{Interaction, InteractionResponseType};
use serenity::model::gateway::Ready;
use serenity::model::id::GuildId;
use serenity::prelude::*;
use shuttle_secrets::SecretStore;
use sqlx::{Executor, PgPool};
use tracing::{error, info};

mod db;

struct Bot {
    database: PgPool,
    guild_id: String,
}

#[async_trait]
impl EventHandler for Bot {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let user_id: i64 = interaction
            .clone()
            .application_command()
            .unwrap()
            .user
            .id
            .into();

        if let Interaction::ApplicationCommand(command) = interaction {
            info!("Received command interaction: {:#?}", command);

            let content = match command.data.name.as_str() {
                "todo" => {
                    let command = command.data.options.get(0).expect("Expected command");

                    // if the todo subcommand has a CommandOption the command is either `add` or `complete`
                    if let Some(subcommand) = command.options.get(0) {
                        match subcommand.resolved.as_ref().expect("Valid subcommand") {
                            CommandDataOptionValue::String(note) => {
                                db::add(&self.database, note, user_id).await.unwrap()
                            }
                            CommandDataOptionValue::Integer(index) => {
                                db::complete(&self.database, index, user_id)
                                    .await
                                    .unwrap_or_else(|_| {
                                        "Please submit a valid index from your todo list"
                                            .to_string()
                                    })
                            }
                            _ => "Please enter a valid todo".to_string(),
                        }
                    // if the todo subcommand doesn't have a CommandOption the command is `list`
                    } else {
                        db::list(&self.database, user_id).await.unwrap()
                    }
                }
                _ => "Command not implemented".to_string(),
            };

            if let Err(why) = command
                .create_interaction_response(&ctx.http, |response| {
                    response
                        .kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|message| message.content(content))
                })
                .await
            {
                error!("Cannot respond to slash command: {}", why);
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        let guild_id = GuildId(self.guild_id.parse().unwrap());

        let _ = GuildId::set_application_commands(&guild_id, &ctx.http, |commands| {
            commands.create_application_command(|command| {
                command
                    .name("todo")
                    .description("Add, list and complete todos")
                    .create_option(|option| {
                        option
                            .name("add")
                            .description("Add a new todo")
                            .kind(CommandOptionType::SubCommand)
                            .create_sub_option(|option| {
                                option
                                    .name("note")
                                    .description("The todo note to add")
                                    .kind(CommandOptionType::String)
                                    .min_length(2)
                                    .max_length(100)
                                    .required(true)
                            })
                    })
                    .create_option(|option| {
                        option
                            .name("complete")
                            .description("The todo to complete")
                            .kind(CommandOptionType::SubCommand)
                            .create_sub_option(|option| {
                                option
                                    .name("index")
                                    .description("The index of the todo to complete")
                                    .kind(CommandOptionType::Integer)
                                    .min_int_value(1)
                                    .required(true)
                            })
                    })
                    .create_option(|option| {
                        option
                            .name("list")
                            .description("List your todos")
                            .kind(CommandOptionType::SubCommand)
                    })
            })
        })
        .await;
    }
}

#[shuttle_service::main]
async fn serenity(
    #[shuttle_shared_db::Postgres] pool: PgPool,
    #[shuttle_secrets::Secrets] secret_store: SecretStore,
) -> shuttle_service::ShuttleSerenity {
    // Get the discord token set in `Secrets.toml`
    let token = secret_store
        .get("DISCORD_TOKEN")
        .context("'DISCORD_TOKEN' was not found")?;
    // Get the guild_id set in `Secrets.toml`
    let guild_id = secret_store
        .get("GUILD_ID")
        .context("'GUILD_ID' was not found")?;

    // Run the schema migration
    pool.execute(include_str!("../schema.sql"))
        .await
        .context("failed to run migrations")?;

    let bot = Bot {
        database: pool,
        guild_id,
    };
    let client = Client::builder(&token, GatewayIntents::empty())
        .event_handler(bot)
        .await
        .expect("Err creating client");

    Ok(client)
}

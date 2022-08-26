use serenity::async_trait;
use serenity::model::prelude::*;
use serenity::prelude::*;
use shuttle_service::error::CustomError;
use shuttle_service::SecretStore;
use sqlx::{Executor, FromRow, PgPool};
use tracing::info;

struct Bot {
    database: PgPool,
}

#[derive(FromRow)]
struct Todo {
    pub id: i32,
    pub note: String,
}

#[async_trait]
impl EventHandler for Bot {
    async fn message(&self, ctx: Context, msg: Message) {
        // The user_id of the user sending a command
        let user_id = msg.author.id.0 as i64;

        // Add a new todo using `~todo add <note>` and persist it in postgres
        if let Some(note) = msg.content.strip_prefix("~todo add") {
            let note = note.trim();
            sqlx::query("INSERT INTO todos (note, user_id) VALUES ($1, $2)")
                .bind(note)
                .bind(user_id)
                .execute(&self.database)
                .await
                .unwrap();

            let response = format!("Added `{}` to your todo list", note);
            msg.channel_id.say(&ctx, response).await.unwrap();

        // Remove a todo by calling `~todo remove <index>` with the index of the todo you want to remove
        // from the `~todo list` output
        } else if let Some(todo_index) = msg.content.strip_prefix("~todo remove") {
            let todo_index = todo_index.trim().parse::<i64>().unwrap() - 1;

            let todo: Todo = sqlx::query_as(
                "SELECT id, note FROM todos WHERE user_id = $1 ORDER BY id LIMIT 1 OFFSET $2",
            )
            .bind(user_id)
            .bind(todo_index)
            .fetch_one(&self.database)
            .await
            .unwrap();

            sqlx::query("DELETE FROM todos WHERE id = $1")
                .bind(todo.id)
                .execute(&self.database)
                .await
                .unwrap();

            let response = format!("Completed `{}`!", todo.note);
            msg.channel_id.say(&ctx, response).await.unwrap();

        // List the calling users todos using ´~todo list`
        } else if msg.content.trim() == "~todo list" {
            let todos: Vec<Todo> =
                sqlx::query_as("SELECT note, id FROM todos WHERE user_id = $1 ORDER BY id")
                    .bind(user_id)
                    .fetch_all(&self.database)
                    .await
                    .unwrap();

            let mut response = format!("You have {} pending todos:\n", todos.len());
            for (i, todo) in todos.iter().enumerate() {
                response += &format!("{}. {}\n", i + 1, todo.note);
            }

            msg.channel_id.say(&ctx, response).await.unwrap();
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
    }
}

#[shuttle_service::main]
async fn serenity(#[shared::Postgres] pool: PgPool) -> shuttle_service::ShuttleSerenity {
    // Get the discord token set in `Secrets.toml` from the shared Postgres database
    let token = pool
        .get_secret("DISCORD_TOKEN")
        .await
        .map_err(CustomError::new)?;

    // Run the schema migration
    pool.execute(include_str!("../schema.sql"))
        .await
        .map_err(CustomError::new)?;

    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let bot = Bot { database: pool };
    let client = Client::builder(&token, intents)
        .event_handler(bot)
        .await
        .expect("Err creating client");

    Ok(client)
}

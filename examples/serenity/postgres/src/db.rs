use sqlx::{FromRow, PgPool};
use std::fmt::Write;

#[derive(FromRow)]
struct Todo {
    pub id: i32,
    pub note: String,
}

pub(crate) async fn add(pool: &PgPool, note: &str, user_id: i64) -> Result<String, sqlx::Error> {
    sqlx::query("INSERT INTO todos (note, user_id) VALUES ($1, $2)")
        .bind(note)
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(format!("Added `{}` to your todo list", note))
}

pub(crate) async fn complete(
    pool: &PgPool,
    index: &i64,
    user_id: i64,
) -> Result<String, sqlx::Error> {
    let todo: Todo = sqlx::query_as(
        "SELECT id, note FROM todos WHERE user_id = $1 ORDER BY id LIMIT 1 OFFSET $2",
    )
    .bind(user_id)
    .bind(index - 1)
    .fetch_one(pool)
    .await?;

    sqlx::query("DELETE FROM todos WHERE id = $1")
        .bind(todo.id)
        .execute(pool)
        .await?;

    Ok(format!("Completed `{}`!", todo.note))
}

pub(crate) async fn list(pool: &PgPool, user_id: i64) -> Result<String, sqlx::Error> {
    let todos: Vec<Todo> =
        sqlx::query_as("SELECT note, id FROM todos WHERE user_id = $1 ORDER BY id")
            .bind(user_id)
            .fetch_all(pool)
            .await?;

    let mut response = format!("You have {} pending todos:\n", todos.len());
    for (i, todo) in todos.iter().enumerate() {
        writeln!(&mut response, "{}. {}", i + 1, todo.note).unwrap();
    }

    Ok(response)
}

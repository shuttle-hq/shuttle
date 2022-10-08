use serde::{Deserialize, Serialize};
use shuttle_service::{error::CustomError, ShuttleTide};
use sqlx::{Executor, FromRow, PgPool};
use tide::{Body, Request};

async fn retrieve(req: Request<MyState>) -> tide::Result {
    let id: i32 = req.param("id")?.parse()?;
    let todo: Todo = sqlx::query_as("SELECT * FROM todos WHERE id = $1")
        .bind(id)
        .fetch_one(&req.state().pool)
        .await?;

    Body::from_json(&todo).map(Into::into)
}

async fn add(mut req: Request<MyState>) -> tide::Result {
    let data: TodoNew = req.body_json().await?;
    let todo: Todo = sqlx::query_as("INSERT INTO todos(note) VALUES ($1) RETURNING id, note")
        .bind(&data.note)
        .fetch_one(&req.state().pool)
        .await?;

    Body::from_json(&todo).map(Into::into)
}

#[derive(Clone)]
struct MyState {
    pool: PgPool,
}

#[shuttle_service::main]
async fn tide(#[shuttle_aws_rds::Postgres] pool: PgPool) -> ShuttleTide<MyState> {
    pool.execute(include_str!("../schema.sql"))
        .await
        .map_err(CustomError::new)?;

    let state = MyState { pool };
    let mut app = tide::with_state(state);

    app.with(tide::log::LogMiddleware::new());
    app.at("/todo").post(add);
    app.at("/todo/:id").get(retrieve);

    Ok(app)
}

#[derive(Deserialize)]
struct TodoNew {
    pub note: String,
}

#[derive(Serialize, FromRow)]
struct Todo {
    pub id: i32,
    pub note: String,
}

#[macro_use]
extern crate rocket;

use rocket::{response::status::BadRequest, serde::json::Json, Build, Rocket, State};
use serde::{Deserialize, Serialize};
use shuttle_service::Factory;
use sqlx::{Executor, FromRow, PgPool};

#[macro_use]
extern crate shuttle_service;

#[get("/<id>")]
async fn retrieve(id: i32, state: &State<MyState>) -> Result<Json<Todo>, BadRequest<String>> {
    let todo = sqlx::query_as("SELECT * FROM todos WHERE id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| BadRequest(Some(e.to_string())))?;

    Ok(Json(todo))
}

#[post("/", data = "<data>")]
async fn add(
    data: Json<TodoNew>,
    state: &State<MyState>,
) -> Result<Json<Todo>, BadRequest<String>> {
    let todo = sqlx::query_as("INSERT INTO todos(note) VALUES ($1) RETURNING id, note")
        .bind(&data.note)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| BadRequest(Some(e.to_string())))?;

    Ok(Json(todo))
}

struct MyState {
    pool: PgPool,
}

async fn wrapper(factory: &mut dyn Factory) -> Result<Rocket<Build>, shuttle_service::Error> {
    let connection_string = factory.get_sql_connection_string().await?;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .min_connections(1)
        .max_connections(5)
        .connect(&connection_string)
        .await?;

    rocket(pool).await?;
}

declare_service!(wrapper);

async fn rocket(pool: PgPool) -> Result<Rocket<Build>, shuttle_service::Error> {
    pool.execute(include_str!("../schema.sql")).await?;

    let state = MyState { pool };
    let rocket = rocket::build()
        .mount("/todo", routes![retrieve, add])
        .manage(state);

    Ok(rocket)
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

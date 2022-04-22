#[macro_use]
extern crate rocket;

use shuttle_service::SecretStore;
use shuttle_service::error::CustomError;
use rocket::response::status::BadRequest;
use rocket::serde::json::Json;
use rocket::{Build, Rocket, State};
use serde::{Deserialize, Serialize};
use sqlx::{Executor, FromRow, PgPool};

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

#[get("/secret")]
async fn secret(state: &State<MyState>) -> Result<String, BadRequest<String>> {
    state.pool.get_secret("MY_API_KEY").await.map_err(|e| BadRequest(Some(e.to_string())))
}

struct MyState {
    pool: PgPool,
}

#[shuttle_service::main]
async fn rocket(pool: PgPool) -> Result<Rocket<Build>, shuttle_service::Error> {
    pool.execute(include_str!("../schema.sql"))
        .await
        .map_err(CustomError::new)?;

    pool.set_secret("MY_API_KEY", "foobar").await;

    let state = MyState { pool };
    let rocket = rocket::build()
        .mount("/todo", routes![retrieve, add, secret])
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

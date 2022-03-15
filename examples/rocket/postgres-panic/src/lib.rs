#[macro_use]
extern crate rocket;

use rocket::{response::status::BadRequest, serde::json::Json, Build, Rocket, State};
use serde::Serialize;
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

struct MyState {
    pool: PgPool,
}

fn rocket() -> Rocket<Build> {
    rocket::build().mount("/todo", routes![retrieve])
}

async fn build_state(factory: &mut dyn Factory) -> Result<MyState, shuttle_service::Error> {
    let connection_string = factory.get_sql_connection_string().await?;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .min_connections(1)
        .max_connections(5)
        .connect(&connection_string)
        .await?;

    pool.execute(include_str!("../schema.sql")).await?;

    let state = MyState { pool };

    Ok(state)
}

declare_service!(Rocket<Build>, rocket, build_state);

#[derive(Serialize, FromRow)]
struct Todo {
    pub id: i32,
    pub note: String,
}

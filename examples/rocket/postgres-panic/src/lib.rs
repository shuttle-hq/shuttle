#[macro_use]
extern crate rocket;

use rocket::{response::status::BadRequest, serde::json::Json, Build, Rocket, State};
use serde::Serialize;
use shuttle_service::Factory;
use sqlx::{FromRow, PgPool};

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

async fn build_state(_factory: &mut dyn Factory) -> Result<MyState, shuttle_service::Error> {
    panic!("no launch pad");
}

declare_service!(Rocket<Build>, rocket, build_state);

#[derive(Serialize, FromRow)]
struct Todo {
    pub id: i32,
    pub note: String,
}

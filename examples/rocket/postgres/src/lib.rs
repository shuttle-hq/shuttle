#[macro_use]
extern crate rocket;

use rocket::{response::status::BadRequest, serde::json::Json, State};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use unveil_service::{declare_service, Deployment, Factory, Service};

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

#[derive(Default)]
struct App;

#[async_trait]
impl<F: Factory> Service<F> for App {
    async fn deploy(&self, factory: &F) -> Result<Deployment, unveil_service::Error> {
        let pool = factory.get_postgres_connection_pool("todo").await?;
        let state = MyState { pool };

        let rocket = rocket::build()
            .manage(state)
            .mount("/todo", routes![retrieve, add])
            .into();

        Ok(rocket)
    }
}

declare_service!(App, App::default);

#[derive(Deserialize)]
struct TodoNew {
    pub note: String,
}

#[derive(Serialize, FromRow)]
struct Todo {
    pub id: i32,
    pub note: String,
}

use poem::{
    error::BadRequest,
    get, handler,
    middleware::AddData,
    post,
    web::{Data, Json, Path},
    EndpointExt, Result, Route,
};
use serde::{Deserialize, Serialize};
use shuttle_service::error::CustomError;
use sqlx::{Executor, FromRow, PgPool};

#[handler]
async fn retrieve(Path(id): Path<i32>, state: Data<&PgPool>) -> Result<Json<Todo>> {
    let todo = sqlx::query_as("SELECT * FROM todos WHERE id = $1")
        .bind(id)
        .fetch_one(state.0)
        .await
        .map_err(BadRequest)?;

    Ok(Json(todo))
}

#[handler]
async fn add(Json(data): Json<TodoNew>, state: Data<&PgPool>) -> Result<Json<Todo>> {
    let todo = sqlx::query_as("INSERT INTO todos(note) VALUES ($1) RETURNING id, note")
        .bind(&data.note)
        .fetch_one(state.0)
        .await
        .map_err(BadRequest)?;

    Ok(Json(todo))
}

#[shuttle_service::main]
async fn main(
    #[shuttle_shared_db::Postgres] pool: PgPool,
) -> shuttle_service::ShuttlePoem<impl poem::Endpoint> {
    pool.execute(include_str!("../schema.sql"))
        .await
        .map_err(CustomError::new)?;

    let app = Route::new()
        .at("/todo", post(add))
        .at("/todo/:id", get(retrieve))
        .with(AddData::new(pool));

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

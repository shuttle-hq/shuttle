#[macro_use]
extern crate rocket;

use rocket::{
    http::Status,
    response::{status, Redirect},
    routes, State,
};
use serde::Serialize;
use shuttle_service::{error::CustomError, ShuttleRocket};
use sqlx::migrate::Migrator;
use sqlx::{FromRow, PgPool};
use url::Url;

struct AppState {
    pool: PgPool,
}

#[derive(Serialize, FromRow)]
struct StoredURL {
    pub id: String,
    pub url: String,
}

#[get("/<id>")]
async fn redirect(id: String, state: &State<AppState>) -> Result<Redirect, status::Custom<String>> {
    let stored_url: StoredURL = sqlx::query_as("SELECT * FROM urls WHERE id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(|err| match err {
            sqlx::Error::RowNotFound => status::Custom(
                Status::NotFound,
                "the requested shortened URL does not exist".into(),
            ),
            _ => status::Custom(
                Status::InternalServerError,
                "something went wrong, sorry 🤷".into(),
            ),
        })?;

    Ok(Redirect::to(stored_url.url))
}

#[post("/", data = "<url>")]
async fn shorten(url: String, state: &State<AppState>) -> Result<String, status::Custom<String>> {
    let id = &nanoid::nanoid!(6);

    let parsed_url = Url::parse(&url).map_err(|err| {
        status::Custom(
            Status::UnprocessableEntity,
            format!("url validation failed: {err}"),
        )
    })?;

    sqlx::query("INSERT INTO urls(id, url) VALUES ($1, $2)")
        .bind(id)
        .bind(parsed_url.as_str())
        .execute(&state.pool)
        .await
        .map_err(|_| {
            status::Custom(
                Status::InternalServerError,
                "something went wrong, sorry 🤷".into(),
            )
        })?;

    Ok(format!("https://s.shuttleapp.rs/{id}"))
}

static MIGRATOR: Migrator = sqlx::migrate!();

#[shuttle_service::main]
async fn rocket(#[shared::Postgres] pool: PgPool) -> ShuttleRocket {
    MIGRATOR.run(&pool).await.map_err(CustomError::new)?;

    let state = AppState { pool };
    let rocket = rocket::build()
        .mount("/", routes![redirect, shorten])
        .manage(state);

    Ok(rocket)
}

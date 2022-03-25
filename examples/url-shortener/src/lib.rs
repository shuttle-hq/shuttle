#[macro_use]
extern crate rocket;

use rocket::{
    http::Status,
    response::{status, Redirect},
    routes, Build, Rocket, State,
};
use serde::Serialize;
use shuttle_service::Factory;
use sqlx::migrate::Migrator;
use sqlx::{FromRow, PgPool};
use url::Url;

#[macro_use]
extern crate shuttle_service;

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

fn rocket() -> Rocket<Build> {
    rocket::build().mount("/", routes![redirect, shorten])
}

static MIGRATOR: Migrator = sqlx::migrate!();

async fn build_state(factory: &mut dyn Factory) -> Result<AppState, shuttle_service::Error> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect(&factory.get_sql_connection_string().await?)
        .await?;

    MIGRATOR
        .run(&pool)
        .await
        .map_err(|err| shuttle_service::Error::Database(sqlx::Error::Migrate(Box::new(err))))?;

    Ok(AppState { pool })
}

declare_service!(Rocket<Build>, rocket, build_state);

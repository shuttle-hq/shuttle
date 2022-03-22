#[macro_use]
extern crate rocket;

use rocket::{
    response::{status::BadRequest, Redirect},
    routes, Build, Rocket, State,
};
use serde::Serialize;
use shuttle_service::Factory;
use sqlx::{Executor, FromRow, PgPool};

#[macro_use]
extern crate shuttle_service;

struct AppState {
    pool: PgPool,
}

#[derive(Serialize, FromRow)]
struct Url {
    pub id: String,
    pub url: String,
}

#[get("/<id>")]
async fn redirect(id: String, state: &State<AppState>) -> Result<Redirect, BadRequest<String>> {
    let url: Url = sqlx::query_as("SELECT * FROM urls WHERE id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| BadRequest(Some(e.to_string())))?;

    Ok(Redirect::to(url.url))
}

#[post("/", data = "<url>")]
async fn shorten(url: String, state: &State<AppState>) -> Result<String, BadRequest<String>> {
    let id = &nanoid::nanoid!(6);

    sqlx::query("INSERT INTO urls(id, url) VALUES ($1, $2)")
        .bind(id)
        .bind(&url)
        .execute(&state.pool)
        .await
        .map_err(|e| BadRequest(Some(e.to_string())))?;

    Ok(format!("https://s.shuttleapp.rs/{id}"))
}

fn rocket() -> Rocket<Build> {
    rocket::build().mount("/", routes![redirect, shorten])
}

async fn build_state(factory: &mut dyn Factory) -> Result<AppState, shuttle_service::Error> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect(&factory.get_sql_connection_string().await?)
        .await?;

    pool.execute(include_str!("../schema.sql")).await?;

    Ok(AppState { pool })
}

declare_service!(Rocket<Build>, rocket, build_state);

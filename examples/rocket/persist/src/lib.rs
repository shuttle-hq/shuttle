#[macro_use]
extern crate rocket;

use rocket::response::status::BadRequest;
use rocket::serde::json::Json;
use rocket::State;
use serde::{Deserialize, Serialize};

use shuttle_persist::PersistInstance;

#[derive(Serialize, Deserialize, Clone)]
struct Weather {
    date: String,
    temp_high: f32,
    temp_low: f32,
    precipitation: f32,
}

struct MyState {
    persist: PersistInstance,
}

#[post("/", data = "<data>")]
async fn add(
    data: Json<Weather>,
    state: &State<MyState>,
) -> Result<Json<Weather>, BadRequest<String>> {
    // Change data Json<Weather> to Weather
    let weather: Weather = data.into_inner();

    let _state = state
        .persist
        .save::<Weather>(
            format!("weather_{}", &weather.date.as_str()).as_str(),
            weather.clone(),
        )
        .map_err(|e| BadRequest(Some(e.to_string())))?;
    Ok(Json(weather))
}

#[get("/<date>")]
async fn retrieve(
    date: String,
    state: &State<MyState>,
) -> Result<Json<Weather>, BadRequest<String>> {
    let weather = state
        .persist
        .load::<Weather>(format!("weather_{}", &date).as_str())
        .map_err(|e| BadRequest(Some(e.to_string())))?;
    Ok(Json(weather))
}

#[shuttle_service::main]
async fn rocket(
    #[shuttle_persist::Persist] persist: PersistInstance,
) -> shuttle_service::ShuttleRocket {
    let state = MyState { persist };
    let rocket = rocket::build()
        .mount("/", routes![retrieve, add])
        .manage(state);

    Ok(rocket)
}

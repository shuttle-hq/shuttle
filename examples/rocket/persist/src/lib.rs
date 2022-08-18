#[macro_use]
extern crate rocket;

use rocket::response::status::BadRequest;
use rocket::serde::{Deserialize, Serialize, json::Json};
use rocket::State;
use shuttle_service::error::CustomError;

use shuttle_service::PersistInstance;

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


// This enpoint is used to create a new weather record.
#[post("/", data = "<data>")]
async fn add(
    data: Json<Weather>,
    state: &State<MyState>,
) -> Result<Json<Weather>, BadRequest<String>> {

    // Change data Json<Weather> to Weather
    let weather: Weather = data.into_inner();

    let state = state
        .persist
        .save::<Weather>(format!("weather_{}", &weather.date.as_str()).as_str(), weather.clone())
        .map_err(|e| BadRequest(Some(e.to_string())))?;
    Ok(Json(weather))
}

// This endpoint is used to retrieve the weather data for a specific date.
#[get("/<date>")]
async fn retrieve(date: String, state: &State<MyState>) -> Result<Json<Weather>, BadRequest<String>> {
    let weather = state
        .persist
        .load::<Weather>(format!("weather_{}", &date).as_str())
        .map_err(|e| BadRequest(Some(e.to_string())))?;
    Ok(Json(weather))
}



#[shuttle_service::main]
async fn rocket(#[persist::Persist] persist: PersistInstance) -> shuttle_service::ShuttleRocket {

    let state = MyState { persist };
    let rocket = rocket::build()
        .mount("/", routes![retrieve, add])
        .manage(state);
    

    Ok(rocket)
}


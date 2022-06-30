#[shuttle_service::main]
async fn rocket() -> shuttle_service::ShuttleRocket {
    let rocket = rocket::build();
    Ok(rocket)
}

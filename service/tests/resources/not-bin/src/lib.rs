// This will fail to compile since it's a library.

#[shuttle_runtime::main]
async fn rocket() -> shuttle_rocket::ShuttleRocket {
    let rocket = rocket::build();
    Ok(rocket.into())
}

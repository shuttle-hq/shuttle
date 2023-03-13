#[macro_use]
extern crate rocket;

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[shuttle_runtime::main]
async fn rocket() -> shuttle_rocket::ShuttleRocket {
    let rocket = rocket::build().mount("/hello", routes![index]);
    Ok(rocket.into())
}

#[cfg(test)]
mod tests {
    #[test]
    fn this_fails() {
        assert!(false);
    }
}

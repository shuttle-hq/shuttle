## Shuttle service integration for the Rocket web framework

### Example

```rust,no_run
use rocket::{get, routes};

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[shuttle_runtime::main]
async fn rocket() -> shuttle_rocket::ShuttleRocket {
    let rocket = rocket::build().mount("/", routes![index]);

    Ok(rocket.into())
}
```

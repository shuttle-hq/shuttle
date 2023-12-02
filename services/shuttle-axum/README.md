## Shuttle service integration for the Axum web framework.

### Example

```rust
#[cfg(feature = "axum")]
use axum::{routing::get, Router};
#[cfg(feature = "axum-0-7")]
use axum_0_7::{routing::get, Router};

async fn hello_world() -> &'static str {
    "Hello, world!"
}

#[shuttle_runtime::main]
async fn axum() -> shuttle_axum::ShuttleAxum {
    let router = Router::new().route("/", get(hello_world));

    Ok(router.into())
}
```

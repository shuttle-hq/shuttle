## Shuttle service integration for the Poem web framework.

Poem 1.* is used by default.

Poem 2.* is supported by using these feature flags:

```toml,ignore
poem = "2.0.0"
shuttle-poem = { version = "0.36.1", default-features = false, features = ["poem-2"] }
```

### Example

```rust,no_run
use poem::{get, handler, Route};
use shuttle_poem::ShuttlePoem;

#[handler]
fn hello_world() -> &'static str {
    "Hello, world!"
}

#[shuttle_runtime::main]
async fn poem() -> ShuttlePoem<impl poem::Endpoint> {
    let app = Route::new().at("/", get(hello_world));

    Ok(app.into())
}
```

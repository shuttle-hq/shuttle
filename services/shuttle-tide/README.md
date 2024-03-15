## Shuttle service integration for the Tide web framework

### Example

```rust,no_run
#[shuttle_runtime::main]
async fn tide() -> shuttle_tide::ShuttleTide<()> {
    let mut app = tide::new();
    app.with(tide::log::LogMiddleware::new());

    app.at("/").get(|_| async { Ok("Hello, world!") });

    Ok(app.into())
}
```

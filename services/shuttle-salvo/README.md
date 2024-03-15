## Shuttle service integration for the Salvo web framework

### Example

```rust,no_run
use salvo::prelude::*;

#[handler]
async fn hello_world(res: &mut Response) {
    res.render(Text::Plain("Hello, world!"));
}

#[shuttle_runtime::main]
async fn salvo() -> shuttle_salvo::ShuttleSalvo {
    let router = Router::new().get(hello_world);

    Ok(router.into())
}
```

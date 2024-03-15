## Shuttle service integration for the Thruster web framework

### Example

```rust,no_run
use thruster::{
    context::basic_hyper_context::{generate_context, BasicHyperContext as Ctx, HyperRequest},
    m, middleware_fn, App, HyperServer, MiddlewareNext, MiddlewareResult, ThrusterServer,
};

#[middleware_fn]
async fn hello(mut context: Ctx, _next: MiddlewareNext<Ctx>) -> MiddlewareResult<Ctx> {
    context.body("Hello, World!");
    Ok(context)
}

#[shuttle_runtime::main]
async fn thruster() -> shuttle_thruster::ShuttleThruster<HyperServer<Ctx, ()>> {
    let server = HyperServer::new(
        App::<HyperRequest, Ctx, ()>::create(generate_context, ()).get("/", m![hello]),
    );

    Ok(server.into())
}
```

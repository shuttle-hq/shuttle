use thruster::{
    context::basic_hyper_context::{generate_context, BasicHyperContext as Ctx, HyperRequest},
    m, middleware_fn, App, HyperServer, MiddlewareNext, MiddlewareResult, ThrusterServer,
};

#[middleware_fn]
async fn hello(mut context: Ctx, _next: MiddlewareNext<Ctx>) -> MiddlewareResult<Ctx> {
    context.body("Hello, World!");
    Ok(context)
}

#[shuttle_service::main]
async fn thruster() -> shuttle_service::ShuttleThruster<HyperServer<Ctx, ()>> {
    Ok(HyperServer::new(
        App::<HyperRequest, Ctx, ()>::create(generate_context, ()).get("/hello", m![hello]),
    ))
}

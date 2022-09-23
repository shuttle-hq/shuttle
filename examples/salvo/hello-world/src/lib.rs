use salvo::prelude::*;

#[handler]
async fn hello_world(res: &mut Response) {
    res.render(Text::Plain("Hello, world!"));
}

#[shuttle_service::main]
async fn salvo() -> shuttle_service::ShuttleSalvo {
    let router = Router::with_path("hello").get(hello_world);

    Ok(router)
}

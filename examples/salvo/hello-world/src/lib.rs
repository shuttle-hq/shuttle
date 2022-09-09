use salvo::prelude::*;

#[handler]
async fn hello_world(res: &mut Response) {
    res.render(Text::Plain("Hello, World!"));
}

#[shuttle_service::main]
async fn salvo() -> shuttle_service::ShuttleSalvo {
    let router = Router::new().get(hello_world);

    Ok(router)
}

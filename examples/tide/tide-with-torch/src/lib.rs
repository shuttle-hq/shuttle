use tch::Tensor;

async fn hello_torch(mut _req: tide::Request<()>) -> tide::Result {
    let t = Tensor::of_slice(&[3, 1, 4, 1, 5]);
    let t = t * 2;
    Ok(format!("Hello with Rust Torch: {:?}", t).into())
}

#[shuttle_service::main]
async fn tide() -> Result<tide::Server<()>, shuttle_service::Error> {
    let mut app = tide::new();

    app.at("/torch").get(hello_torch);

    Ok(app)
}

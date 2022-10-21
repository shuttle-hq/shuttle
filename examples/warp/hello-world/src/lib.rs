use warp::Filter;
use warp::Reply;

#[shuttle_service::main]
async fn warp() -> shuttle_service::ShuttleWarp<(impl Reply,)> {
    let route = warp::any().map(|| "Hello, World!");
    Ok(route.boxed())
}

use warp::Filter;
use warp::Reply;

#[shuttle_service::main]
//async fn warp() -> shuttle_service::ShuttleWarp<(impl Reply,)> {
async fn warp() -> shuttle_service::ShuttleWarp<(impl Reply,)> {
    let route = warp::any().map(|| "Hello From Warp!");
    Ok(route.boxed())
}

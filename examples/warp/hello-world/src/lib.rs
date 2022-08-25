use warp::Filter;

#[shuttle_service::main]
async fn warp() -> shuttle_service::ShuttleWarp<impl warp::Filter> {
    //let router = warp::path("hello")
    ////.and(warp::path::param())
    //.and(warp::header("user-agent"))
    //.map(|_: _| format!("Hello warp!"));

    let hi = warp::path("hello")
        .and(warp::path::param())
        .and(warp::header("user-agent"))
        .map(|param: String, agent: String| format!("Hello {}, whose agent is {}", param, agent));
    //let route = warp::any().map(|| "Hello From Warp!");
    //let router = warp::service(route);

    Ok(hi)
}

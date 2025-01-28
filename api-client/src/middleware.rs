use http::Extensions;
use reqwest::{Request, Response};
use reqwest_middleware::{Middleware, Next};
use tracing::debug;

pub struct LoggingMiddleware;

#[async_trait::async_trait]
impl Middleware for LoggingMiddleware {
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<Response> {
        debug!("Request: {} {}", req.method(), req.url());
        let res = next.run(req, extensions).await;
        match res {
            Ok(ref res) => {
                debug!("Response: {}", res.status());
            }
            Err(ref e) => {
                debug!("Response error: {}", e);
            }
        }
        res
    }
}

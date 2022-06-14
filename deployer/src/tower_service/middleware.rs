use log::log;

pub struct LoggingLayer(pub log::Level);

impl<S> tower::Layer<S> for LoggingLayer {
    type Service = LoggingService<S>;

    fn layer(&self, service: S) -> Self::Service {
        LoggingService {
            level: self.0,
            service,
        }
    }
}

#[derive(Clone)]
pub struct LoggingService<S> {
    level: log::Level,
    service: S,
}

impl<Body, S: tower::Service<http::Request<Body>>> tower::Service<http::Request<Body>>
    for LoggingService<S>
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<Body>) -> Self::Future {
        let host = req
            .headers()
            .get("host")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("unknown");

        log!(
            self.level,
            "Incoming {} request from '{}' for: {}",
            req.method(),
            host,
            req.uri()
        );

        self.service.call(req)
    }
}

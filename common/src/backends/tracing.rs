use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use http::{Request, Response};
use opentelemetry::{
    global,
    runtime::Tokio,
    sdk::{propagation::TraceContextPropagator, trace, Resource},
    KeyValue,
};
use opentelemetry_http::{HeaderExtractor, HeaderInjector};
use opentelemetry_otlp::WithExportConfig;
use pin_project::pin_project;
use tower::{Layer, Service};
use tracing::{debug_span, instrument::Instrumented, Instrument, Span, Subscriber};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{fmt, prelude::*, registry::LookupSpan, EnvFilter};

use super::future::ResponseFuture;

pub fn setup_tracing<S>(subscriber: S, service_name: &str)
where
    S: Subscriber + for<'a> LookupSpan<'a> + Send + Sync,
{
    global::set_text_map_propagator(TraceContextPropagator::new());

    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();
    let fmt_layer = fmt::layer();

    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint("http://otel-collector:4317"),
        )
        .with_trace_config(
            trace::config().with_resource(Resource::new(vec![KeyValue::new(
                "service.name",
                service_name.to_string(),
            )])),
        )
        .install_batch(Tokio)
        .unwrap();
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    subscriber
        .with(filter_layer)
        .with(fmt_layer)
        .with(otel_layer)
        .init();
}

/// Layer to extract tracing from headers and set the context on the current span
#[derive(Clone)]
pub struct ExtractPropagationLayer;

impl<S> Layer<S> for ExtractPropagationLayer {
    type Service = ExtractPropagation<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ExtractPropagation { inner }
    }
}

/// Middleware for extracting tracing propagation info and setting them on the currently active span
#[derive(Clone)]
pub struct ExtractPropagation<S> {
    inner: S,
}

#[pin_project]
pub struct ExtractPropagationFuture<F> {
    #[pin]
    response_future: F,
}

impl<F, Body, Error> Future for ExtractPropagationFuture<F>
where
    F: Future<Output = Result<Response<Body>, Error>>,
{
    type Output = Result<Response<Body>, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        match this.response_future.poll(cx) {
            Poll::Ready(result) => match result {
                Ok(response) => {
                    Span::current().record("http.status_code", response.status().as_u16());

                    Poll::Ready(Ok(response))
                }
                other => Poll::Ready(other),
            },

            Poll::Pending => Poll::Pending,
        }
    }
}

impl<S, Body, ResponseBody> Service<Request<Body>> for ExtractPropagation<S>
where
    S: Service<Request<Body>, Response = Response<ResponseBody>> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = ExtractPropagationFuture<Instrumented<S::Future>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let span = debug_span!(
            "request",
            http.uri = %req.uri(),
            http.method = %req.method(),
            http.status_code = tracing::field::Empty,
        );

        let parent_context = global::get_text_map_propagator(|propagator| {
            propagator.extract(&HeaderExtractor(req.headers()))
        });

        span.set_parent(parent_context);

        let response_future = self.inner.call(req).instrument(span);

        ExtractPropagationFuture { response_future }
    }
}

/// This layer adds the current tracing span to any outgoing request
#[derive(Clone)]
pub struct InjectPropagationLayer;

impl<S> Layer<S> for InjectPropagationLayer {
    type Service = InjectPropagation<S>;

    fn layer(&self, inner: S) -> Self::Service {
        InjectPropagation { inner }
    }
}

#[derive(Clone)]
pub struct InjectPropagation<S> {
    inner: S,
}

impl<S, Body> Service<Request<Body>> for InjectPropagation<S>
where
    S: Service<Request<Body>> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = ResponseFuture<S::Future>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        let cx = Span::current().context();

        global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&cx, &mut HeaderInjector(req.headers_mut()))
        });

        let future = self.inner.call(req);

        ResponseFuture(future)
    }
}

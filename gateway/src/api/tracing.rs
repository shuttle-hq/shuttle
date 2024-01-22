use axum::{extract::Path, http::Request, middleware::Next, response::Response};

/// Layer to correctly set the tracing parameters of a :project_name route
pub(crate) async fn project_name_tracing_layer<B>(
    // Ideally we would use a custom struct containing `project_name`, but Axum rejects it for some
    // reason.
    Path(paths): Path<Vec<(String, String)>>,
    request: Request<B>,
    next: Next<B>,
) -> Response {
    if let Some((_, project_name)) = paths
        .into_iter()
        .find(|(path_name, _)| path_name == "project_name")
    {
        let current_span = tracing::Span::current();
        current_span.record("shuttle.project.name", project_name);
    }
    next.run(request).await
}

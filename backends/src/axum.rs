use async_trait::async_trait;
use axum::extract::path::ErrorKind;
use axum::{
    extract::{rejection::PathRejection, FromRequestParts},
    http::request::Parts,
};
use http::StatusCode;
use serde::de::DeserializeOwned;

use shuttle_common::models::error::ApiError;

/// Custom `Path` extractor that customizes the error from `axum::extract::Path`.
///
/// Prints the custom error message if deserialization resulted in a custom de::Error,
/// which is what the [`crate::project_name::ProjectName`] parser uses.
pub struct CustomErrorPath<T>(pub T);

impl<T> core::ops::Deref for CustomErrorPath<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> core::ops::DerefMut for CustomErrorPath<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[async_trait]
impl<S, T> FromRequestParts<S> for CustomErrorPath<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match axum::extract::Path::<T>::from_request_parts(parts, state).await {
            Ok(value) => Ok(Self(value.0)),
            Err(rejection) => {
                if let PathRejection::FailedToDeserializePathParams(inner) = &rejection {
                    if let ErrorKind::Message(message) = inner.kind() {
                        return Err(ApiError {
                            message: message.clone(),
                            status_code: StatusCode::BAD_REQUEST.as_u16(),
                        });
                    }
                }

                Err(ApiError {
                    message: rejection.body_text(),
                    status_code: rejection.status().as_u16(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use axum::http::StatusCode;
    use axum::{body::Body, routing::get, Router};
    use http::Request;
    use tower::Service;

    use crate::project_name::ProjectName;

    #[tokio::test]
    async fn project_name_paths() {
        let mut app =
            Router::new()
                .route(
                    "/:project_name",
                    get(
                        |CustomErrorPath(project_name): CustomErrorPath<ProjectName>| async move {
                            project_name.to_string()
                        },
                    ),
                )
                .route(
                    "/:project_name/:num",
                    get(
                        |CustomErrorPath((project_name, num)): CustomErrorPath<(
                            ProjectName,
                            u8,
                        )>| async move { format!("{project_name} {num}") },
                    ),
                );

        let response = app
            .call(Request::get("/test123").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert_eq!(&body[..], b"test123");

        let response = app
            .call(Request::get("/__test123").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert!(&body[..].starts_with(br#"{"message":"Invalid project name"#));

        let response = app
            .call(Request::get("/test123/123").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert_eq!(&body[..], b"test123 123");

        let response = app
            .call(Request::get("/test123/asdf").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert!(&body[..].starts_with(br#"{"message":"Invalid URL"#));
    }
}

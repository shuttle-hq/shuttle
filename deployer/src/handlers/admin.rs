use async_trait::async_trait;
use axum::{
    extract::FromRequest,
    headers::{authorization::Bearer, Authorization},
    http::StatusCode,
    TypedHeader,
};

/// Guard used to make sure a request has the correct admin token set on the Auth Bearer
///
/// *Note*
/// This guard requires the [AdminSecret] extension to be set
pub struct AdminGuard;

#[async_trait]
impl<B> FromRequest<B> for AdminGuard
where
    B: Send,
{
    type Rejection = (StatusCode, String);

    async fn from_request(
        req: &mut axum::extract::RequestParts<B>,
    ) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) =
            TypedHeader::<Authorization<Bearer>>::from_request(req)
                .await
                .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
        let admin_secret = req
            .extensions()
            .get::<AdminSecret>()
            .expect("AdminSecret to be available on extensions");

        if bearer.token() == admin_secret.0 {
            Ok(Self)
        } else {
            Err((
                StatusCode::FORBIDDEN,
                "request could not be authorized".to_string(),
            ))
        }
    }
}

#[derive(Clone)]
pub struct AdminSecret(pub String);

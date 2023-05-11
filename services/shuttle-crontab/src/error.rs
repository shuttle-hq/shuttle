use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use shuttle_persist::PersistError;

#[derive(Debug)]
pub struct CrontabServiceError(PersistError);

impl From<PersistError> for CrontabServiceError {
    fn from(err: PersistError) -> Self {
        Self(err)
    }
}

impl IntoResponse for CrontabServiceError {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR).into_response()
    }
}

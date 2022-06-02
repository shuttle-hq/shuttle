use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("failed to create role '{0}")]
    CreateRole(String),

    #[error("failed to update role '{0}")]
    UpdateRole(String),

    #[error("failed to create DB '{0}")]
    CreateDB(String),

    #[error("unexpected error '{0}")]
    Unexpected(#[from] sqlx::Error),
}

use aws_sdk_rds::{
    error::{CreateDBInstanceError, DescribeDBInstancesError},
    types::SdkError,
};
use thiserror::Error;
use tonic::Status;
use tracing::error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("failed to create role: {0}")]
    CreateRole(String),

    #[error("failed to update role: {0}")]
    UpdateRole(String),

    #[error("failed to create DB: {0}")]
    CreateDB(String),

    #[error("unexpected sqlx error: {0}")]
    UnexpectedSqlx(#[from] sqlx::Error),

    #[error("unexpected mongodb error: {0}")]
    UnexpectedMongodb(#[from] mongodb::error::Error),

    #[error("failed to create RDS instance: {0}")]
    CreateRDSInstance(#[from] SdkError<CreateDBInstanceError>),

    #[error("failed to get description of RDS instance: {0}")]
    DescribeRDSInstance(#[from] SdkError<DescribeDBInstancesError>),

    #[error["plain error: {0}"]]
    Plain(String),
}

unsafe impl Send for Error {}

impl From<Error> for Status {
    fn from(err: Error) -> Self {
        error!(error = &err as &dyn std::error::Error, "provision failed");
        Status::internal("failed to provision a database")
    }
}

use aws_sdk_rds::{
    error::SdkError,
    operation::{
        create_db_instance::CreateDBInstanceError, delete_db_instance::DeleteDBInstanceError,
        describe_db_instances::DescribeDBInstancesError,
    },
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
    #[error("failed to drop role: {0}")]
    DeleteRole(String),
    #[error("failed to create DB: {0}")]
    CreateDB(String),
    #[error("failed to drop DB: {0}")]
    DeleteDB(String),
    #[error("unexpected sqlx error: {0}")]
    UnexpectedSqlx(#[from] sqlx::Error),
    #[error("unexpected mongodb error: {0}")]
    UnexpectedMongodb(#[from] mongodb::error::Error),
    #[error("failed to create RDS instance: {0}")]
    CreateRDSInstance(#[from] SdkError<CreateDBInstanceError>),
    #[error("failed to get description of RDS instance: {0}")]
    DescribeRDSInstance(#[from] SdkError<DescribeDBInstancesError>),
    #[error("failed to delete RDS instance: {0}")]
    DeleteRDSInstance(#[from] SdkError<DeleteDBInstanceError>),
    #[error["plain error: {0}"]]
    Plain(String),
}

unsafe impl Send for Error {}

impl From<Error> for Status {
    fn from(err: Error) -> Self {
        error!(error = &err as &dyn std::error::Error, "provision failed");

        let message = match err {
            Error::CreateRDSInstance(_) | Error::CreateDB(_) | Error::CreateRole(_) => {
                "failed to provision a database"
            }
            Error::DeleteDB(_) | Error::DeleteRole(_) | Error::DeleteRDSInstance(_) => {
                "failed to delete a database"
            }
            _ => "an unexpected error occurred",
        };

        Status::internal(message)
    }
}

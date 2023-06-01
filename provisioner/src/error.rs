use aws_sdk_iam::operation::{
    attach_user_policy::AttachUserPolicyError, create_access_key::CreateAccessKeyError,
    create_policy::CreatePolicyError, create_user::CreateUserError,
    delete_access_key::DeleteAccessKeyError, delete_policy::DeletePolicyError,
    delete_user::DeleteUserError, detach_user_policy::DetachUserPolicyError,
};
use aws_sdk_rds::{
    error::SdkError,
    operation::{
        create_db_instance::CreateDBInstanceError, describe_db_instances::DescribeDBInstancesError,
    },
};
use aws_sdk_sts::operation::get_caller_identity::GetCallerIdentityError;
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

    #[error("failed to create IAM policy for AWS: {0}")]
    CreateIAMPolicy(#[from] CreatePolicyError),

    #[error("failed to create IAM user for AWS: {0}")]
    CreateIAMUser(#[from] CreateUserError),

    #[error("failed to get IAM identity keys for AWS: {0}")]
    GetIAMIdentityKeys(#[from] std::io::Error),

    #[error("failed to delete a DynamoDB table in AWS: {0}")]
    DeleteDynamoDBTableError(#[from] Box<dyn std::error::Error>),

    #[error("failed to delete IAM user access key for AWS: {0}")]
    DeleteAccessKey(#[from] SdkError<DeleteAccessKeyError>),

    #[error("failed to delete IAM user for AWS: {0}")]
    DeleteIAMUser(#[from] SdkError<DeleteUserError>),

    #[error("failed to get caller identity for AWS: {0}")]
    GetCallerIdentity(#[from] SdkError<GetCallerIdentityError>),

    #[error("failed to get caller account for AWS: {0}")]
    GetAccount(String),

    #[error("failed to delete IAM policy for AWS: {0}")]
    DeleteIAMPolicy(#[from] SdkError<DeletePolicyError>),

    #[error("failed to create access key for AWS: {0}")]
    CreateAccessKey(#[from] SdkError<CreateAccessKeyError>),

    #[error("failed to get access key for AWS: {0}")]
    GetAccessKey(String),

    #[error("failed to get access key id for AWS: {0}")]
    GetAccessKeyId(String),

    #[error("failed to get secret access key for AWS: {0}")]
    GetSecretAccessKey(String),

    #[error("failed to attach user policy for AWS: {0}")]
    AttachUserPolicy(#[from] SdkError<AttachUserPolicyError>),

    #[error("failed to detach user policy for AWS: {0}")]
    DetachUserPolicy(#[from] SdkError<DetachUserPolicyError>),

    #[error("failed to get region for AWS: {0}")]
    GetRegion(String),

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

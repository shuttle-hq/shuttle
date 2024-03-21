use async_trait::async_trait;
use http::header::AUTHORIZATION;
use shuttle_proto::resource_recorder::Client;
use shuttle_proto::resource_recorder::ProjectResourcesRequest;
use tracing::instrument;

use shuttle_common::{database, resource};

use super::Error;

/// DAL for access resources data of projects
#[async_trait]
pub trait ResourceDal: Send {
    /// Get the resources belonging to a project
    async fn get_project_resources(
        &mut self,
        project_id: &str,
        token: &str,
    ) -> Result<Vec<resource::Response>, Error>;

    /// Get only the RDS resources that belong to a project
    async fn get_project_rds_resources(
        &mut self,
        project_id: &str,
        token: &str,
    ) -> Result<Vec<resource::Response>, Error> {
        let rds_resources = self
            .get_project_resources(project_id, token)
            .await?
            .into_iter()
            .filter(|r| {
                matches!(
                    r.r#type,
                    resource::Type::Database(database::Type::AwsRds(_))
                )
            })
            .collect();

        Ok(rds_resources)
    }
}

#[async_trait]
impl<T> ResourceDal for &mut T
where
    T: ResourceDal,
{
    #[instrument(skip_all, fields(shuttle.project.id = project_id))]
    async fn get_project_resources(
        &mut self,
        project_id: &str,
        token: &str,
    ) -> Result<Vec<resource::Response>, Error> {
        (**self).get_project_resources(project_id, token).await
    }
}

#[async_trait]
impl ResourceDal for Client {
    async fn get_project_resources(
        &mut self,
        project_id: &str,
        token: &str,
    ) -> Result<Vec<shuttle_common::resource::Response>, Error> {
        let mut req = tonic::Request::new(ProjectResourcesRequest {
            project_id: project_id.to_string(),
        });

        req.metadata_mut().insert(
            AUTHORIZATION.as_str(),
            format!("Bearer {token}")
                .parse()
                .expect("to construct a bearer token"),
        );

        let resp = (*self)
            .get_project_resources(req)
            .await?
            .into_inner()
            .clone();

        let resources = resp
            .resources
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error: anyhow::Error| tonic::Status::internal(error.to_string()))?;

        Ok(resources)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;
    use shuttle_common::{database, resource};
    use shuttle_proto::resource_recorder::{get_client, record_request, Client, RecordRequest};
    use test_context::{test_context, AsyncTestContext};
    use tonic::Request;

    use crate::test_utils::resource_recorder::get_mocked_resource_recorder;

    struct Wrap(Client);

    impl AsyncTestContext for Wrap {
        async fn setup() -> Self {
            let port = get_mocked_resource_recorder().await;

            Self(get_client(format!("http://localhost:{port}").parse().unwrap()).await)
        }

        async fn teardown(self) {}
    }

    #[test_context(Wrap)]
    #[tokio::test]
    async fn get_project_resources(r_r_client: &mut Wrap) {
        // First record some resources
        r_r_client
            .0
            .record_resources(Request::new(RecordRequest {
                project_id: "project_1".to_string(),
                service_id: "service_1".to_string(),
                resources: vec![
                    record_request::Resource {
                        r#type: "database::shared::postgres".to_string(),
                        config: serde_json::to_vec(&json!({"public": true})).unwrap(),
                        data: serde_json::to_vec(&json!({"username": "test"})).unwrap(),
                    },
                    record_request::Resource {
                        r#type: "database::aws_rds::mariadb".to_string(),
                        config: serde_json::to_vec(&json!({})).unwrap(),
                        data: serde_json::to_vec(&json!({"username": "maria"})).unwrap(),
                    },
                ],
            }))
            .await
            .unwrap();

        let resources = (&mut r_r_client.0 as &mut dyn ResourceDal)
            .get_project_resources("project_1", "user-1")
            .await
            .unwrap();

        assert_eq!(
            resources,
            vec![
                resource::Response {
                    r#type: resource::Type::Database(database::Type::Shared(
                        database::SharedEngine::Postgres
                    )),
                    config: json!({"public": true}),
                    data: json!({"username": "test"}),
                },
                resource::Response {
                    r#type: resource::Type::Database(database::Type::AwsRds(
                        database::AwsRdsEngine::MariaDB
                    )),
                    config: json!({}),
                    data: json!({"username": "maria"}),
                }
            ]
        );

        // Getting only RDS resources should filter correctly
        let resources = (&mut r_r_client.0 as &mut dyn ResourceDal)
            .get_project_rds_resources("project_1", "user-1")
            .await
            .unwrap();

        assert_eq!(
            resources,
            vec![resource::Response {
                r#type: resource::Type::Database(database::Type::AwsRds(
                    database::AwsRdsEngine::MariaDB
                )),
                config: json!({}),
                data: json!({"username": "maria"}),
            }]
        );
    }
}

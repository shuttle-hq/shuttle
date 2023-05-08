use async_trait::async_trait;
use dal::{Dal, Resource};
use shuttle_proto::resource_recorder::{
    resource_recorder_server::ResourceRecorder, resources_response, ProjectResourcesRequest,
    RecordRequest, RecordResponse, ResourcesResponse, ServiceResourcesRequest,
};
use tonic::{Request, Response, Status};

pub mod args;
mod dal;
mod r#type;

pub use dal::Sqlite;
use tracing::error;
use ulid::DecodeError;

/// A wrapper to capture any error possible with this service
enum Error<DE: std::error::Error> {
    UlidDecode(DecodeError),
    Dal(DE),
    String(String),
}

impl<DE: std::error::Error> ToString for Error<DE> {
    fn to_string(&self) -> String {
        match self {
            Error::UlidDecode(error) => format!("could not decode id: {error}"),
            Error::Dal(error) => {
                error!(error = error.to_string(), "database request failed");

                format!("failed to interact with recorder")
            }
            Error::String(error) => format!("could not parse resource type: {error}"),
        }
    }
}

pub struct Service<D> {
    dal: D,
}

impl<D> Service<D>
where
    D: Dal + Send + Sync + 'static,
{
    pub fn new(dal: D) -> Self {
        Self { dal }
    }

    /// Record the addition of a new resource
    async fn add(&self, request: RecordRequest) -> Result<(), Error<D::Error>> {
        self.dal
            .add_resources(
                request.project_id.parse().map_err(Error::UlidDecode)?,
                request.service_id.parse().map_err(Error::UlidDecode)?,
                request
                    .resources
                    .into_iter()
                    .map(TryInto::<Resource>::try_into)
                    .collect::<Result<_, _>>()
                    .map_err(Error::String)?,
            )
            .await
            .map_err(Error::Dal)?;

        Ok(())
    }

    /// Get the resources that below to a project
    async fn project_resources(
        &self,
        project_id: String,
    ) -> Result<Vec<resources_response::Resource>, Error<D::Error>> {
        let resources = self
            .dal
            .get_project_resources(project_id.parse().map_err(Error::UlidDecode)?)
            .await
            .map_err(Error::Dal)?;

        Ok(resources.into_iter().map(Into::into).collect())
    }

    /// Get the resources that below to a service
    async fn service_resources(
        &self,
        service_id: String,
    ) -> Result<Vec<resources_response::Resource>, Error<D::Error>> {
        let resources = self
            .dal
            .get_service_resources(service_id.parse().map_err(Error::UlidDecode)?)
            .await
            .map_err(Error::Dal)?;

        Ok(resources.into_iter().map(Into::into).collect())
    }
}

#[async_trait]
impl<D> ResourceRecorder for Service<D>
where
    D: Dal + Send + Sync + 'static,
{
    async fn record_resources(
        &self,
        request: Request<RecordRequest>,
    ) -> Result<Response<RecordResponse>, Status> {
        let request = request.into_inner();
        let result = match self.add(request).await {
            Ok(()) => RecordResponse {
                success: true,
                message: Default::default(),
            },
            Err(e) => RecordResponse {
                success: false,
                message: e.to_string(),
            },
        };

        Ok(Response::new(result))
    }

    async fn get_project_resources(
        &self,
        request: Request<ProjectResourcesRequest>,
    ) -> Result<Response<ResourcesResponse>, Status> {
        let request = request.into_inner();
        let result = match self.project_resources(request.project_id).await {
            Ok(resources) => ResourcesResponse {
                success: true,
                message: Default::default(),
                resources,
            },
            Err(e) => ResourcesResponse {
                success: false,
                message: e.to_string(),
                resources: Vec::new(),
            },
        };

        Ok(Response::new(result))
    }

    async fn get_service_resources(
        &self,
        request: Request<ServiceResourcesRequest>,
    ) -> Result<Response<ResourcesResponse>, Status> {
        let request = request.into_inner();
        let result = match self.service_resources(request.service_id).await {
            Ok(resources) => ResourcesResponse {
                success: true,
                message: Default::default(),
                resources,
            },
            Err(e) => ResourcesResponse {
                success: false,
                message: e.to_string(),
                resources: Vec::new(),
            },
        };

        Ok(Response::new(result))
    }
}

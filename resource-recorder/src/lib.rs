use async_trait::async_trait;
use dal::{Dal, Resource};
use shuttle_proto::resource_recorder::{
    resource_recorder_server::ResourceRecorder, RecordRequest, RecordResult,
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
}

#[async_trait]
impl<D> ResourceRecorder for Service<D>
where
    D: Dal + Send + Sync + 'static,
{
    async fn record_resources(
        &self,
        request: Request<RecordRequest>,
    ) -> Result<Response<RecordResult>, Status> {
        let request = request.into_inner();
        let result = match self.add(request).await {
            Ok(()) => RecordResult {
                success: true,
                message: Default::default(),
            },
            Err(e) => RecordResult {
                success: false,
                message: e.to_string(),
            },
        };

        Ok(Response::new(result))
    }
}

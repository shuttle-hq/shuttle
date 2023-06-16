use async_trait::async_trait;
use opentelemetry_proto::tonic::collector::logs::v1::{
    logs_service_server::LogsService, ExportLogsServiceRequest, ExportLogsServiceResponse,
};
use tonic::{Request, Response, Status};

pub struct ShuttleLogsOtlp;

impl ShuttleLogsOtlp {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl LogsService for ShuttleLogsOtlp {
    async fn export(
        &self,
        request: Request<ExportLogsServiceRequest>,
    ) -> Result<Response<ExportLogsServiceResponse>, Status> {
        let request = request.into_inner();

        println!("{request:#?}");

        Ok(Response::new(ExportLogsServiceResponse {
            partial_success: None,
        }))
    }
}

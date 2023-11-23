use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Mutex,
};

use portpicker::pick_unused_port;
use shuttle_proto::resource_recorder::{
    resource_recorder_server::{ResourceRecorder, ResourceRecorderServer},
    ProjectResourcesRequest, RecordRequest, Resource, ResourceIds, ResourceResponse,
    ResourcesResponse, ResultResponse, ServiceResourcesRequest,
};
use tonic::{async_trait, transport::Server, Request, Response, Status};

struct MockedResourceRecorder {
    resources: Mutex<Vec<Resource>>,
}

#[async_trait]
impl ResourceRecorder for MockedResourceRecorder {
    async fn record_resources(
        &self,
        request: Request<RecordRequest>,
    ) -> Result<Response<ResultResponse>, Status> {
        println!("recording resources");
        let RecordRequest {
            project_id,
            service_id,
            resources,
        } = request.into_inner();

        let mut resources = resources
            .into_iter()
            .map(|r| Resource {
                project_id: project_id.clone(),
                service_id: service_id.clone(),
                r#type: r.r#type,
                config: r.config,
                data: r.data,
                is_active: true,
                created_at: None,
                last_updated: None,
            })
            .collect();

        self.resources.lock().unwrap().append(&mut resources);

        Ok(Response::new(ResultResponse {
            success: true,
            message: Default::default(),
        }))
    }

    async fn get_project_resources(
        &self,
        _request: Request<ProjectResourcesRequest>,
    ) -> Result<Response<ResourcesResponse>, Status> {
        println!("getting project resources");
        Ok(Response::new(Default::default()))
    }

    async fn get_service_resources(
        &self,
        request: Request<ServiceResourcesRequest>,
    ) -> Result<Response<ResourcesResponse>, Status> {
        println!("getting service resources");

        let ServiceResourcesRequest { service_id } = request.into_inner();
        let resources = self
            .resources
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.service_id == service_id)
            .cloned()
            .collect();

        Ok(Response::new(ResourcesResponse {
            success: true,
            message: Default::default(),
            resources,
        }))
    }

    async fn get_resource(
        &self,
        _request: tonic::Request<ResourceIds>,
    ) -> Result<Response<ResourceResponse>, Status> {
        println!("getting resources");
        Ok(Response::new(Default::default()))
    }

    async fn delete_resource(
        &self,
        _request: tonic::Request<ResourceIds>,
    ) -> Result<Response<ResultResponse>, Status> {
        Ok(Response::new(Default::default()))
    }
}

/// Start a mocked resource recorder and return the address it started on
pub async fn start_mocked_resource_recorder() -> u16 {
    let resource_recorder = MockedResourceRecorder {
        resources: Mutex::new(Vec::new()),
    };

    let port = pick_unused_port().unwrap();
    let resource_recorder_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), port);
    tokio::spawn(async move {
        Server::builder()
            .add_service(ResourceRecorderServer::new(resource_recorder))
            .serve(resource_recorder_addr)
            .await
    });

    port
}

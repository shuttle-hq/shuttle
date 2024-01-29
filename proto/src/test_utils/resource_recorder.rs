use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Mutex,
};

use portpicker::pick_unused_port;
use tonic::{async_trait, transport::Server, Request, Response, Status};

use crate::generated::resource_recorder::{
    resource_recorder_server::{ResourceRecorder, ResourceRecorderServer},
    ProjectResourcesRequest, RecordRequest, Resource, ResourceIds, ResourceResponse,
    ResourcesResponse, ResultResponse, ServiceResourcesRequest,
};

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
        request: Request<ProjectResourcesRequest>,
    ) -> Result<Response<ResourcesResponse>, Status> {
        println!("getting project resources");

        // Make sure clients set the authorization correctly
        let _user = request
            .metadata()
            .get("authorization")
            .unwrap()
            .to_str()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap();

        let ProjectResourcesRequest { project_id } = request.into_inner();
        let resources = self
            .resources
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.project_id == project_id)
            .cloned()
            .collect();

        Ok(Response::new(ResourcesResponse {
            success: true,
            message: Default::default(),
            resources,
        }))
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
        request: tonic::Request<ResourceIds>,
    ) -> Result<Response<ResourceResponse>, Status> {
        println!("getting resource");

        let ResourceIds {
            project_id,
            service_id,
            r#type,
        } = request.into_inner();
        let resource = self
            .resources
            .lock()
            .unwrap()
            .iter()
            .find(|r| {
                r.project_id == project_id && r.service_id == service_id && r.r#type == r#type
            })
            .cloned();

        Ok(Response::new(ResourceResponse {
            success: resource.is_some(),
            message: Default::default(),
            resource,
        }))
    }

    async fn delete_resource(
        &self,
        request: tonic::Request<ResourceIds>,
    ) -> Result<Response<ResultResponse>, Status> {
        println!("delete resource");

        let ResourceIds {
            project_id,
            service_id,
            r#type,
        } = request.into_inner();

        // Fail to delete a metadata resource if requested
        if r#type == "metadata" {
            return Ok(Response::new(ResultResponse {
                success: false,
                message: Default::default(),
            }));
        }

        self.resources.lock().unwrap().retain(|r| {
            !(r.project_id == project_id && r.service_id == service_id && r.r#type == r#type)
        });

        Ok(Response::new(ResultResponse {
            success: true,
            message: Default::default(),
        }))
    }
}

/// Start a mocked resource recorder and return the port it started on
/// This mock will function like a normal resource recorder. However, it will always fail to delete metadata resources
/// if any tests need to simulate a failure.
pub async fn get_mocked_resource_recorder() -> u16 {
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

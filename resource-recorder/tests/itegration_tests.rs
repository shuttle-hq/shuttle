use std::net::{Ipv4Addr, SocketAddr};

use portpicker::pick_unused_port;
use pretty_assertions::assert_eq;
use serde_json::json;
use shuttle_proto::resource_recorder::{
    record_request, resource_recorder_client::ResourceRecorderClient,
    resource_recorder_server::ResourceRecorderServer, ProjectResourcesRequest, RecordRequest,
    Resource, ResourcesResponse, ResultResponse, ServiceResourcesRequest,
};
use shuttle_resource_recorder::{Service, Sqlite};
use tokio::select;
use tonic::{transport::Server, Request};
use ulid::Ulid;

#[tokio::test]
async fn manage_resources() {
    let port = pick_unused_port().unwrap();
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);

    let server_future = async {
        Server::builder()
            .add_service(ResourceRecorderServer::new(Service::new(
                Sqlite::new_in_memory().await,
            )))
            .serve(addr)
            .await
            .unwrap()
    };

    let test_future = async {
        // Make sure the server starts first
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;

        let mut client = ResourceRecorderClient::connect(format!("http://localhost:{port}"))
            .await
            .unwrap();

        let project_id = Ulid::new().to_string();
        let service_id = Ulid::new().to_string();

        // Add resources for on service
        let response = client
            .record_resources(Request::new(RecordRequest {
                project_id: project_id.clone(),
                service_id: service_id.clone(),
                resources: vec![
                    record_request::Resource {
                        r#type: "database::shared::postgres".to_string(),
                        config: serde_json::to_vec(&json!({"public": true})).unwrap(),
                        data: serde_json::to_vec(&json!({"username": "test"})).unwrap(),
                    },
                    record_request::Resource {
                        r#type: "secrets".to_string(),
                        config: serde_json::to_vec(&json!({})).unwrap(),
                        data: serde_json::to_vec(&json!({"password": "brrrr"})).unwrap(),
                    },
                ],
            }))
            .await
            .unwrap()
            .into_inner();

        let expected = ResultResponse {
            success: true,
            message: String::new(),
        };

        assert_eq!(response, expected);

        // Add resources for another service on same project
        let service_id2 = Ulid::new().to_string();

        let response = client
            .record_resources(Request::new(RecordRequest {
                project_id: project_id.clone(),
                service_id: service_id2.clone(),
                resources: vec![record_request::Resource {
                    r#type: "static_folder".to_string(),
                    config: serde_json::to_vec(&json!({"folder": "static"})).unwrap(),
                    data: serde_json::to_vec(&json!({"path": "/tmp/static"})).unwrap(),
                }],
            }))
            .await
            .unwrap()
            .into_inner();

        assert_eq!(response, expected);

        // Add resources to a new project
        let project_id2 = Ulid::new().to_string();
        let service_id3 = Ulid::new().to_string();

        let response = client
            .record_resources(Request::new(RecordRequest {
                project_id: project_id2,
                service_id: service_id3,
                resources: vec![record_request::Resource {
                    r#type: "static_folder".to_string(),
                    config: serde_json::to_vec(&json!({"folder": "publi"})).unwrap(),
                    data: serde_json::to_vec(&json!({"path": "/tmp/publi"})).unwrap(),
                }],
            }))
            .await
            .unwrap()
            .into_inner();

        assert_eq!(response, expected);

        // Fetching resources for a service
        let response = client
            .get_service_resources(Request::new(ServiceResourcesRequest {
                service_id: service_id.clone(),
            }))
            .await
            .unwrap()
            .into_inner();

        let expected = ResourcesResponse {
            success: true,
            message: String::new(),
            resources: vec![
                Resource {
                    project_id: project_id.clone(),
                    service_id: service_id.clone(),
                    r#type: "database::shared::postgres".to_string(),
                    config: serde_json::to_vec(&json!({"public": true})).unwrap(),
                    data: serde_json::to_vec(&json!({"username": "test"})).unwrap(),
                    is_active: true,
                    created_at: response.resources[0].created_at.clone(),
                },
                Resource {
                    project_id: project_id.clone(),
                    service_id: service_id.clone(),
                    r#type: "secrets".to_string(),
                    config: serde_json::to_vec(&json!({})).unwrap(),
                    data: serde_json::to_vec(&json!({"password": "brrrr"})).unwrap(),
                    is_active: true,
                    created_at: response.resources[1].created_at.clone(),
                },
            ],
        };

        assert_eq!(response, expected);

        // Fetching resources for a project
        let response = client
            .get_project_resources(Request::new(ProjectResourcesRequest {
                project_id: project_id.clone(),
            }))
            .await
            .unwrap()
            .into_inner();

        let expected = ResourcesResponse {
            success: true,
            message: String::new(),
            resources: vec![
                Resource {
                    project_id: project_id.clone(),
                    service_id: service_id.clone(),
                    r#type: "database::shared::postgres".to_string(),
                    config: serde_json::to_vec(&json!({"public": true})).unwrap(),
                    data: serde_json::to_vec(&json!({"username": "test"})).unwrap(),
                    is_active: true,
                    created_at: response.resources[0].created_at.clone(),
                },
                Resource {
                    project_id: project_id.clone(),
                    service_id: service_id.clone(),
                    r#type: "secrets".to_string(),
                    config: serde_json::to_vec(&json!({})).unwrap(),
                    data: serde_json::to_vec(&json!({"password": "brrrr"})).unwrap(),
                    is_active: true,
                    created_at: response.resources[1].created_at.clone(),
                },
                Resource {
                    project_id: project_id.clone(),
                    service_id: service_id2.clone(),
                    r#type: "static_folder".to_string(),
                    config: serde_json::to_vec(&json!({"folder": "static"})).unwrap(),
                    data: serde_json::to_vec(&json!({"path": "/tmp/static"})).unwrap(),
                    is_active: true,
                    created_at: response.resources[2].created_at.clone(),
                },
            ],
        };

        assert_eq!(response, expected);

        // Deleting a resource
        let response = client
            .delete_resource(Request::new(Resource {
                project_id: project_id.clone(),
                service_id: service_id2.clone(),
                r#type: "static_folder".to_string(),
                config: serde_json::to_vec(&json!({"folder": "static"})).unwrap(),
                data: serde_json::to_vec(&json!({"path": "/tmp/static"})).unwrap(),
                is_active: true,
                created_at: response.resources[2].created_at.clone(),
            }))
            .await
            .unwrap()
            .into_inner();

        let expected = ResultResponse {
            success: true,
            message: String::new(),
        };

        assert_eq!(response, expected);

        // Updating resources on a service
        let response = client
            .record_resources(Request::new(RecordRequest {
                project_id: project_id.clone(),
                service_id: service_id.clone(),
                resources: vec![record_request::Resource {
                    r#type: "database::shared::postgres".to_string(),
                    config: serde_json::to_vec(&json!({"public": false})).unwrap(),
                    data: serde_json::to_vec(&json!({"username": "inner"})).unwrap(),
                }],
            }))
            .await
            .unwrap()
            .into_inner();

        let expected = ResultResponse {
            success: true,
            message: String::new(),
        };

        assert_eq!(response, expected);

        let response = client
            .get_project_resources(Request::new(ProjectResourcesRequest {
                project_id: project_id.clone(),
            }))
            .await
            .unwrap()
            .into_inner();

        let expected = ResourcesResponse {
            success: true,
            message: String::new(),
            resources: vec![
                Resource {
                    project_id: project_id.clone(),
                    service_id: service_id.clone(),
                    r#type: "secrets".to_string(),
                    config: serde_json::to_vec(&json!({})).unwrap(),
                    data: serde_json::to_vec(&json!({"password": "brrrr"})).unwrap(),
                    is_active: false,
                    created_at: response.resources[0].created_at.clone(),
                },
                Resource {
                    project_id: project_id.clone(),
                    service_id: service_id.clone(),
                    r#type: "database::shared::postgres".to_string(),
                    config: serde_json::to_vec(&json!({"public": false})).unwrap(),
                    data: serde_json::to_vec(&json!({"username": "inner"})).unwrap(),
                    is_active: true,
                    created_at: response.resources[1].created_at.clone(),
                },
            ],
        };

        assert_eq!(response, expected);
    };

    select! {
        _ = server_future => panic!("server finished first"),
        _ = test_future => {},
    }
}

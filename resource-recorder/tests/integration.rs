use std::net::{Ipv4Addr, SocketAddr};

use portpicker::pick_unused_port;
use pretty_assertions::{assert_eq, assert_ne};
use serde_json::json;
use shuttle_backends::client::ServicesApiClient;
use shuttle_backends::test_utils::gateway::get_mocked_gateway_server;
use shuttle_common::claims::Scope;
use shuttle_common_tests::JwtScopesLayer;
use shuttle_proto::resource_recorder::{
    record_request, resource_recorder_client::ResourceRecorderClient,
    resource_recorder_server::ResourceRecorderServer, ProjectResourcesRequest, RecordRequest,
    Resource, ResourceIds, ResourcesResponse, ResultResponse,
};
use shuttle_resource_recorder::{Service, Sqlite};
use tokio::select;
use tonic::{transport::Server, Request};

#[tokio::test]
async fn manage_resources() {
    let port = pick_unused_port().unwrap();
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);

    let server = get_mocked_gateway_server().await;
    let client = ServicesApiClient::new(server.uri().parse().unwrap());

    let server_future = async {
        Server::builder()
            .layer(JwtScopesLayer::new(vec![
                Scope::Resources,
                Scope::ResourcesWrite,
            ]))
            .add_service(ResourceRecorderServer::new(Service::new(
                Sqlite::new_in_memory().await,
                client,
            )))
            .serve(addr)
            .await
            .unwrap()
    };

    let test_future = async {
        // Make sure the server starts first
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let mut client = ResourceRecorderClient::connect(format!("http://localhost:{port}"))
            .await
            .unwrap();

        let project_id = "00000000000000000000000001".to_string();
        let service_id = "00000000000000000000000001".to_string();

        let req = Request::new(RecordRequest {
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
        });

        // Add resources for on service
        let response = client.record_resources(req).await.unwrap().into_inner();

        let expected = ResultResponse {
            success: true,
            message: String::new(),
        };

        assert_eq!(response, expected);

        // Add resources for another service on same project
        let service_id2 = "00000000000000000000000002".to_string();

        let response = client
            .record_resources(Request::new(RecordRequest {
                project_id: project_id.clone(),
                service_id: service_id2.clone(),
                resources: vec![record_request::Resource {
                    r#type: "secrets".to_string(),
                    config: serde_json::to_vec(&json!({"folder": "static"})).unwrap(),
                    data: serde_json::to_vec(&json!({"path": "/tmp/static"})).unwrap(),
                }],
            }))
            .await
            .unwrap()
            .into_inner();

        assert_eq!(response, expected);

        // Add resources to a new project
        let project_id2 = "00000000000000000000000002".to_string();
        let service_id3 = "00000000000000000000000003".to_string();

        let response = client
            .record_resources(Request::new(RecordRequest {
                project_id: project_id2,
                service_id: service_id3,
                resources: vec![record_request::Resource {
                    r#type: "secrets".to_string(),
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
            .get_project_resources(Request::new(ProjectResourcesRequest {
                project_id: project_id.clone(),
            }))
            .await
            .unwrap()
            .into_inner();

        let mut service_db = Resource {
            project_id: project_id.clone(),
            service_id: service_id.clone(),
            r#type: "database::shared::postgres".to_string(),
            config: serde_json::to_vec(&json!({"public": true})).unwrap(),
            data: serde_json::to_vec(&json!({"username": "test"})).unwrap(),
            is_active: true,
            created_at: response.resources[0].created_at.clone(),
            last_updated: response.resources[0].last_updated.clone(),
        };
        let mut service_secrets = Resource {
            project_id: project_id.clone(),
            service_id: service_id.clone(),
            r#type: "secrets".to_string(),
            config: serde_json::to_vec(&json!({})).unwrap(),
            data: serde_json::to_vec(&json!({"password": "brrrr"})).unwrap(),
            is_active: true,
            created_at: response.resources[1].created_at.clone(),
            last_updated: response.resources[1].last_updated.clone(),
        };
        let service_secrets2 = Resource {
            project_id: project_id.clone(),
            service_id: service_id2.clone(),
            r#type: "secrets".to_string(),
            config: serde_json::to_vec(&json!({"folder": "static"})).unwrap(),
            data: serde_json::to_vec(&json!({"path": "/tmp/static"})).unwrap(),
            is_active: true,
            created_at: response.resources[2].created_at.clone(),
            last_updated: response.resources[2].last_updated.clone(),
        };

        let expected = ResourcesResponse {
            success: true,
            message: String::new(),
            resources: vec![
                service_db.clone(),
                service_secrets.clone(),
                service_secrets2,
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

        let service2_static_folder = ResourceIds {
            project_id: project_id.clone(),
            service_id: service_id2.clone(),
            r#type: "secrets".to_string(),
        };

        let expected = ResourcesResponse {
            success: true,
            message: String::new(),
            resources: vec![
                service_db.clone(),
                service_secrets.clone(),
                Resource {
                    config: serde_json::to_vec(&json!({"folder": "static"})).unwrap(),
                    data: serde_json::to_vec(&json!({"path": "/tmp/static"})).unwrap(),
                    is_active: true,
                    created_at: response.resources[2].created_at.clone(),
                    last_updated: response.resources[2].last_updated.clone(),
                    project_id: service2_static_folder.project_id.clone(),
                    service_id: service2_static_folder.service_id.clone(),
                    r#type: service2_static_folder.r#type.clone(),
                },
            ],
        };

        assert_eq!(response, expected);

        // Deleting a resource
        let response = client
            .delete_resource(Request::new(service2_static_folder))
            .await
            .unwrap()
            .into_inner();

        let expected = ResultResponse {
            success: true,
            message: String::new(),
        };

        assert_eq!(response, expected);

        // Updating resources on a service
        service_db.config = serde_json::to_vec(&json!({"public": false})).unwrap();
        service_db.data = serde_json::to_vec(&json!({"username": "inner"})).unwrap();

        service_secrets.is_active = false;

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

        assert_ne!(
            response.resources[1].last_updated, service_db.last_updated,
            "should update last_updated"
        );

        service_db
            .last_updated
            .clone_from(&response.resources[1].last_updated);

        let expected = ResourcesResponse {
            success: true,
            message: String::new(),
            resources: vec![service_secrets, service_db],
        };

        assert_eq!(response, expected);
    };

    select! {
        _ = server_future => panic!("server finished first"),
        _ = test_future => {},
    }
}

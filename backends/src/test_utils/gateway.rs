use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use anyhow::Error;
use async_trait::async_trait;
use permit_client_rs::models::{UserRead, UserRole};
use permit_pdp_client_rs::models::UserPermissionsResult;
use serde::Serialize;
use shuttle_common::claims::AccountTier;
use wiremock::{
    http,
    matchers::{method, path, path_regex},
    Mock, MockServer, Request, ResponseTemplate,
};

use crate::client::PermissionsDal;

pub async fn get_mocked_gateway_server() -> MockServer {
    let mock_server = MockServer::start().await;

    let projects = vec![
        Project {
            id: "00000000000000000000000001",
            account_id: "user-1",
            name: "user-1-project-1",
            state: "stopped",
            idle_minutes: 30,
        },
        Project {
            id: "00000000000000000000000002",
            account_id: "user-1",
            name: "user-1-project-2",
            state: "ready",
            idle_minutes: 30,
        },
        Project {
            id: "00000000000000000000000003",
            account_id: "user-2",
            name: "user-2-project-1",
            state: "ready",
            idle_minutes: 30,
        },
    ];

    let p = projects.clone();
    Mock::given(method(http::Method::GET))
        .and(path("/projects"))
        .respond_with(move |req: &Request| {
            let Some(bearer) = req.headers.get("AUTHORIZATION") else {
                return ResponseTemplate::new(401);
            };

            let user = bearer.to_str().unwrap().split_whitespace().nth(1).unwrap();

            let body: Vec<_> = p.iter().filter(|p| p.account_id == user).collect();

            ResponseTemplate::new(200).set_body_json(body)
        })
        .mount(&mock_server)
        .await;

    let p = projects.clone();
    Mock::given(method(http::Method::HEAD))
        .and(path_regex("/projects/[a-z0-9-]+"))
        .respond_with(move |req: &Request| {
            let Some(bearer) = req.headers.get("AUTHORIZATION") else {
                return ResponseTemplate::new(401);
            };
            let project = req.url.path().strip_prefix("/projects/").unwrap();

            let user = bearer.to_str().unwrap().split_whitespace().nth(1).unwrap();

            if p.iter().any(|p| p.account_id == user && p.name == project) {
                ResponseTemplate::new(200)
            } else {
                ResponseTemplate::new(401)
            }
        })
        .mount(&mock_server)
        .await;

    mock_server
}

/// A denormalized project to make it easy to return mocked responses
#[derive(Debug, Clone, Serialize)]
struct Project<'a> {
    id: &'a str,
    account_id: &'a str,
    name: &'a str,
    state: &'a str,
    idle_minutes: u64,
}

#[derive(Clone, Default)]
pub struct PermissionsMock {
    pub users: Arc<RwLock<HashMap<String, UserRead>>>,
}

#[async_trait]
impl PermissionsDal for PermissionsMock {
    async fn get_user(&self, _user_id: &str) -> Result<UserRead, Error> {
        unimplemented!()
    }

    async fn delete_user(&self, _user_id: &str) -> Result<(), Error> {
        unimplemented!()
    }

    async fn new_user(&self, user_id: &str) -> Result<UserRead, Error> {
        let user = UserRead {
            key: user_id.to_string(),
            roles: Some(vec![UserRole {
                role: AccountTier::Basic.to_string(),
                tenant: "default".to_string(),
            }]),
            ..Default::default()
        };

        self.users
            .write()
            .unwrap()
            .insert(user_id.to_string(), user.clone());

        Ok(user)
    }

    async fn make_pro(&self, user_id: &str) -> Result<(), Error> {
        self.users.write().unwrap().get_mut(user_id).unwrap().roles = Some(vec![UserRole {
            role: AccountTier::Pro.to_string(),
            tenant: "default".to_string(),
        }]);

        Ok(())
    }

    async fn make_basic(&self, user_id: &str) -> Result<(), Error> {
        self.users.write().unwrap().get_mut(user_id).unwrap().roles = Some(vec![UserRole {
            role: AccountTier::Basic.to_string(),
            tenant: "default".to_string(),
        }]);

        Ok(())
    }

    async fn create_project(&self, _user_id: &str, _project_id: &str) -> Result<(), Error> {
        unimplemented!()
    }

    async fn delete_project(&self, _project_id: &str) -> Result<(), Error> {
        unimplemented!()
    }

    async fn get_user_projects(&self, _user_id: &str) -> Result<Vec<UserPermissionsResult>, Error> {
        unimplemented!()
    }

    async fn allowed(
        &self,
        _user_id: &str,
        _project_id: &str,
        _action: &str,
    ) -> Result<bool, Error> {
        unimplemented!()
    }
}

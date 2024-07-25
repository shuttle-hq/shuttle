use std::sync::Arc;

use async_trait::async_trait;
use permit_client_rs::models::UserRead;
use serde::Serialize;
use shuttle_common::models::team;
use tokio::sync::Mutex;
use wiremock::{
    http,
    matchers::{method, path, path_regex},
    Mock, MockServer, Request, ResponseTemplate,
};

use crate::client::{
    permit::{Owner, Result, Team},
    PermissionsDal,
};

pub async fn get_mocked_gateway_server() -> MockServer {
    let mock_server = MockServer::start().await;

    let projects = vec![
        Project {
            id: "00000000000000000000000001",
            owner_id: "user-1",
            name: "user-1-project-1",
            state: "stopped",
            idle_minutes: 30,
            is_admin: true,
            owner_type: "user",
        },
        Project {
            id: "00000000000000000000000002",
            owner_id: "user-1",
            name: "user-1-project-2",
            state: "ready",
            idle_minutes: 30,
            is_admin: true,
            owner_type: "user",
        },
        Project {
            id: "00000000000000000000000003",
            owner_id: "user-2",
            name: "user-2-project-1",
            state: "ready",
            idle_minutes: 30,
            is_admin: true,
            owner_type: "user",
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

            let body: Vec<_> = p.iter().filter(|p| p.owner_id == user).collect();

            ResponseTemplate::new(200).set_body_json(body)
        })
        .mount(&mock_server)
        .await;

    let p = projects.clone();
    Mock::given(method(http::Method::HEAD))
        .and(path_regex("/projects/[a-zA-Z0-9-]+"))
        .respond_with(move |req: &Request| {
            let Some(bearer) = req.headers.get("AUTHORIZATION") else {
                return ResponseTemplate::new(401);
            };
            let project = req.url.path().strip_prefix("/projects/").unwrap();

            let user = bearer.to_str().unwrap().split_whitespace().nth(1).unwrap();

            if p.iter()
                .any(|p| p.owner_id == user && (p.name == project || p.id == project))
            {
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
    name: &'a str,
    state: &'a str,
    idle_minutes: u64,
    is_admin: bool,
    owner_type: &'a str,
    owner_id: &'a str,
}

#[derive(Clone, Default)]
pub struct PermissionsMock {
    pub calls: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl PermissionsDal for PermissionsMock {
    async fn get_user(&self, user_id: &str) -> Result<UserRead> {
        self.calls.lock().await.push(format!("get_user {user_id}"));
        Ok(Default::default())
    }

    async fn delete_user(&self, user_id: &str) -> Result<()> {
        self.calls
            .lock()
            .await
            .push(format!("delete_user {user_id}"));
        Ok(())
    }

    async fn new_user(&self, user_id: &str) -> Result<UserRead> {
        self.calls.lock().await.push(format!("new_user {user_id}"));
        Ok(Default::default())
    }

    async fn make_pro(&self, user_id: &str) -> Result<()> {
        self.calls.lock().await.push(format!("make_pro {user_id}"));
        Ok(())
    }

    async fn make_basic(&self, user_id: &str) -> Result<()> {
        self.calls
            .lock()
            .await
            .push(format!("make_basic {user_id}"));
        Ok(())
    }

    async fn create_project(&self, user_id: &str, project_id: &str) -> Result<()> {
        self.calls
            .lock()
            .await
            .push(format!("create_project {user_id} {project_id}"));
        Ok(())
    }

    async fn delete_project(&self, project_id: &str) -> Result<()> {
        self.calls
            .lock()
            .await
            .push(format!("delete_project {project_id}"));
        Ok(())
    }

    async fn get_personal_projects(&self, user_id: &str) -> Result<Vec<String>> {
        self.calls
            .lock()
            .await
            .push(format!("get_personal_projects {user_id}"));
        Ok(vec![])
    }

    async fn allowed(&self, user_id: &str, project_id: &str, action: &str) -> Result<bool> {
        self.calls
            .lock()
            .await
            .push(format!("allowed {user_id} {project_id} {action}"));
        Ok(true)
    }

    async fn create_team(&self, user_id: &str, team: &Team) -> Result<()> {
        self.calls.lock().await.push(format!(
            "create_team {user_id} {} {}",
            team.id, team.display_name
        ));
        Ok(())
    }

    async fn delete_team(&self, user_id: &str, team_id: &str) -> Result<()> {
        self.calls
            .lock()
            .await
            .push(format!("delete_team {user_id} {team_id}"));
        Ok(())
    }

    async fn get_team(&self, user_id: &str, team_id: &str) -> Result<team::Response> {
        self.calls
            .lock()
            .await
            .push(format!("get_team {user_id} {team_id}"));
        Ok(Default::default())
    }

    async fn get_team_projects(&self, user_id: &str, team_id: &str) -> Result<Vec<String>> {
        self.calls
            .lock()
            .await
            .push(format!("get_team_projects {user_id} {team_id}"));
        Ok(Default::default())
    }

    async fn get_teams(&self, user_id: &str) -> Result<Vec<team::Response>> {
        self.calls.lock().await.push(format!("get_teams {user_id}"));
        Ok(Default::default())
    }

    async fn transfer_project_to_user(
        &self,
        user_id: &str,
        project_id: &str,
        new_user_id: &str,
    ) -> Result<()> {
        self.calls.lock().await.push(format!(
            "transfer_project_to_user {user_id} {project_id} {new_user_id}"
        ));

        Ok(())
    }

    async fn transfer_project_to_team(
        &self,
        user_id: &str,
        project_id: &str,
        team_id: &str,
    ) -> Result<()> {
        self.calls.lock().await.push(format!(
            "transfer_project_to_team {user_id} {project_id} {team_id}"
        ));
        Ok(())
    }

    async fn transfer_project_from_team(
        &self,
        user_id: &str,
        project_id: &str,
        team_id: &str,
    ) -> Result<()> {
        self.calls.lock().await.push(format!(
            "transfer_project_from_team {user_id} {project_id} {team_id}"
        ));
        Ok(())
    }

    async fn add_team_member(&self, admin_user: &str, team_id: &str, user_id: &str) -> Result<()> {
        self.calls
            .lock()
            .await
            .push(format!("add_team_member {admin_user} {team_id} {user_id}"));
        Ok(())
    }

    async fn remove_team_member(
        &self,
        admin_user: &str,
        team_id: &str,
        user_id: &str,
    ) -> Result<()> {
        self.calls.lock().await.push(format!(
            "remove_team_member {admin_user} {team_id} {user_id}"
        ));
        Ok(())
    }

    async fn get_team_members(
        &self,
        user_id: &str,
        team_id: &str,
    ) -> Result<Vec<team::MemberResponse>> {
        self.calls
            .lock()
            .await
            .push(format!("get_team_members {user_id} {team_id}"));
        Ok(Default::default())
    }

    async fn get_project_owner(&self, user_id: &str, project_id: &str) -> Result<Owner> {
        self.calls
            .lock()
            .await
            .push(format!("get_project_owner {user_id} {project_id}"));
        Ok(Owner::User(user_id.to_string()))
    }
}

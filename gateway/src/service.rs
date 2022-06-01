use std::net::IpAddr;
use std::path::Path as StdPath;
use std::sync::Arc;

use serde::{Serialize, Deserialize};

use axum::body::Body;
use axum::http::Request;
use axum::response::Response;
use bollard::Docker;
use hyper::client::HttpConnector;
use sqlx::migrate::MigrateDatabase;
use sqlx::sqlite::{
    Sqlite,
    SqlitePool
};
use sqlx::types::Json as SqlxJson;
use sqlx::{
    query,
    Row
};
use tokio::sync::mpsc::{
    channel,
    Sender
};
use hyper::Client as HyperClient;

use super::{
    Context,
    Error,
    ProjectName
};
use crate::project::{
    self,
    Project
};
use crate::{
    Refresh,
    State
};

pub struct Work {
    project_name: ProjectName,
    account_name: AccountName,
    project: Project
}

pub struct GatewayService {
    docker: Docker,
    hyper: HyperClient<HttpConnector, Body>,
    db: SqlitePool,
    work: Sender<Work>
}

const DB_PATH: &'static str = "gateway.sqlite";

use crate::API_PORT;
use crate::{auth::Key, AccountName};
use crate::auth::User;

impl GatewayService {
    pub async fn init() -> Arc<Self> {
        let docker = Docker::connect_with_http_defaults().unwrap();

        let hyper = HyperClient::new();
        if !StdPath::new(DB_PATH).exists() {
            Sqlite::create_database(DB_PATH).await.unwrap();
        }

        let db = SqlitePool::connect(DB_PATH).await.unwrap();

        query("CREATE TABLE IF NOT EXISTS projects (project_name TEXT PRIMARY KEY, account_name TEXT NOT NULL, project_state JSON NOT NULL)")
            .execute(&db)
            .await
            .unwrap();

        query("CREATE TABLE IF NOT EXISTS accounts (account_name TEXT PRIMARY KEY, key TEXT UNIQUE, super_user BOOLEAN DEFAULT FALSE)")
            .execute(&db)
            .await
            .unwrap();

        let (work, mut queue) = channel(256);

        let service = Arc::new(Self {
            docker,
            hyper,
            db,
            work: work.clone()
        });

        let worker = Arc::clone(&service);

        tokio::spawn(async move {
            while let Some(Work {
                project_name,
                project,
                account_name
            }) = queue.recv().await
            {
                let current_state = project.state();
                debug!(
                    "picking up project_name={} in state={}",
                    project_name, current_state
                );
                // TODO panicking loses the worker thread and bricks the service
                let next = project.next(&worker.context()).await.unwrap();
                worker.update_project(&project_name, &next).await.unwrap();
                debug!(
                    "left project_name={} in new state={} for account={}",
                    project_name,
                    next.state(),
                    account_name
                );
                // TODO: replace with is_done
                if current_state != next.state() {
                    debug!(
                        "queuing more work for project_name={} (not done)",
                        project_name
                    );
                    // TODO may deadlock?
                    work.send(Work {
                        project_name,
                        project: next,
                        account_name
                    })
                    .await
                    .ok()
                    .expect("failed to queue work");
                } else {
                    debug!("worker dropping project_name={} (done)", project_name);
                }
            }
        });

        // Queue up all the projects for reconciliation
        for Work {
            project_name,
            project,
            account_name
        } in service.iter_projects().await
        {
            let project = project.refresh(&service.context()).await.unwrap();
            service
                .work
                .send(Work {
                    project_name,
                    project,
                    account_name
                })
                .await
                .ok()
                .expect("failed to queue work");
        }

        service
    }

    pub fn context(&self) -> GatewayContext<'_> {
        GatewayContext {
            docker: &self.docker,
            hyper: &self.hyper,
        }
    }

    pub async fn route(
        &self,
        project_name: &ProjectName,
        route: String,
        req: Request<Body>
    ) -> Result<Response<Body>, Error> {
        let target_ip = self
            .find_project(project_name)
            .await
            .unwrap()
            .target_ip()?
            .unwrap(); // TODO handle
        let resp = hyper_reverse_proxy::call(
            "127.0.0.1".parse().unwrap(),
            &format!("http://{}:{}/{}", target_ip, API_PORT, route),
            req
        )
        .await
        .unwrap();
        Ok(resp)
    }

    async fn iter_projects(&self) -> impl Iterator<Item = Work> {
        query("SELECT * FROM projects")
            .fetch_all(&self.db)
            .await
            .unwrap()
            .into_iter()
            .map(|row| Work {
                project_name: row.get("project_name"),
                project: row.get::<SqlxJson<Project>, _>("project_state").0,
                account_name: row.get("account_name")
            })
    }

    pub async fn find_project(&self, project_name: &ProjectName) -> Option<Project> {
        query("SELECT project_state FROM projects WHERE project_name=?1")
            .bind(project_name)
            .fetch_optional(&self.db)
            .await
            .unwrap()
            .map(|r| {
                r.try_get::<SqlxJson<Project>, _>("project_state")
                    .unwrap()
                    .0
            })
    }

    async fn update_project(
        &self,
        project_name: &ProjectName,
        project: &Project
    ) -> Result<(), Error> {
        query("UPDATE projects SET project_state = ?1 WHERE project_name = ?2")
            .bind(&SqlxJson(project))
            .bind(project_name)
            .execute(&self.db)
            .await
            .unwrap();
        Ok(())
    }

    pub async fn key_from_account_name(&self, account_name: &AccountName) -> Result<Key, Error> {
        let key = query("SELECT key FROM accounts WHERE account_name = ?1")
            .bind(account_name)
            .fetch_optional(&self.db)
            .await
            .unwrap()
            .map(|row| row.try_get("key").unwrap())
            .unwrap();  // TODO: account not found
        Ok(key)
    }

    pub async fn account_name_from_key(&self, key: &Key) -> Result<AccountName, Error> {
        let name = query("SELECT account_name FROM accounts WHERE key = ?1")
            .bind(key)
            .fetch_optional(&self.db)
            .await
            .unwrap()
            .map(|row| row.try_get("account_name").unwrap())
            .unwrap();  // TODO: user not found
        Ok(name)
    }

    pub async fn user_from_account_name(&self, name: AccountName) -> Result<User, Error> {
        let key = self.key_from_account_name(&name).await?;
        let projects = self.iter_user_projects(&name)
            .await
            .collect();
        Ok(User {
            name,
            key,
            projects
        })
    }

    pub async fn user_from_key(&self, key: Key) -> Result<User, Error> {
        let name = self.account_name_from_key(&key)
            .await?;
        let projects = self.iter_user_projects(&name)
            .await
            .collect();
        Ok(User {
            name,
            key,
            projects
        })
    }

    pub async fn create_user(&self, name: AccountName) -> Result<User, Error> {
        let key = Key::new_random();
        query("INSERT INTO accounts (account_name, key) VALUES (?1, ?2)")
            .bind(&name)
            .bind(&key)
            .execute(&self.db)
            .await
            .unwrap(); // TODO: user already exists
        Ok(User {
            name,
            key,
            projects: Vec::default()
        })
    }

    pub async fn is_super_user(&self, account_name: &AccountName) -> Result<bool, Error> {
        let super_user = query("SELECT super_user FROM accounts WHERE account_name = ?1")
            .bind(account_name)
            .fetch_optional(&self.db)
            .await
            .unwrap()
            .map(|row| row.try_get("super_user").unwrap())
            .unwrap();  // TODO: user does not exist
        Ok(super_user)
    }

    async fn iter_user_projects(&self, AccountName(account_name): &AccountName) -> impl Iterator<Item = ProjectName> {
        query("SELECT project_name FROM projects WHERE account_name = ?1")
            .bind(account_name)
            .fetch_all(&self.db)
            .await
            .unwrap()
            .into_iter()
            .map(|row| {
                row.try_get::<ProjectName, _>("project_name").unwrap()
            })
    }

    pub async fn create_project(&self, project_name: ProjectName, account_name: AccountName) -> Result<Project, Error> {
        let project = SqlxJson(Project::Creating(project::ProjectCreating::new(
            project_name.clone()
        )));
        // TODO
        query("INSERT INTO projects (project_name, account_name, project_state) VALUES (?1, ?2, ?3)")
            .bind(&project_name)
            .bind(&account_name)
            .bind(&project)
            .execute(&self.db)
            .await
            .unwrap();
        let project = project.0;
        self.work
            .send(Work {
                project_name,
                project: project.clone(),
                account_name
            })
            .await
            .ok()
            .expect("failed to queue work");
        Ok(project)
    }
}

pub struct GatewayContext<'c> {
    docker: &'c Docker,
    hyper: &'c HyperClient<HttpConnector, Body>
}

impl<'c> Context<'c> for GatewayContext<'c> {
    fn docker(&self) -> &'c Docker {
        self.docker
    }
}

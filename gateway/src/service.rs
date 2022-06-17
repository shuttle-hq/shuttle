use std::fmt::Debug;
use std::net::IpAddr;
use std::panic::catch_unwind;
use std::path::{Path as StdPath, PathBuf};
use std::sync::Arc;

use axum::headers::authorization::Basic;
use axum::headers::{Authorization, Header};
use rand::distributions::{Alphanumeric, DistString};
use serde::{Deserialize, Serialize};

use axum::body::Body;
use axum::http::Request;
use axum::response::Response;
use bollard::Docker;
use hyper::client::HttpConnector;
use hyper::Client as HyperClient;
use sqlx::error::DatabaseError;
use sqlx::migrate::MigrateDatabase;
use sqlx::sqlite::{Sqlite, SqlitePool};
use sqlx::types::Json as SqlxJson;
use sqlx::{query, Error as SqlxError, Row};
use tokio::sync::{
    mpsc::{channel, Receiver, Sender},
    Mutex,
};

use super::{Context, Error, ProjectName};
use crate::args::Args;
use crate::project::{self, Project};
use crate::{EndState, ErrorKind, Refresh, Service, State};

impl From<SqlxError> for Error {
    fn from(err: SqlxError) -> Self {
        debug!("internal SQLx error: {err}");
        Self::source(ErrorKind::Internal, err)
    }
}

#[derive(Debug, Clone)]
pub struct Work<W = Project> {
    project_name: ProjectName,
    account_name: AccountName,
    work: W,
}

#[async_trait]
impl<'c, W> State<'c> for Work<W>
where
    W: State<'c>,
{
    type Next = Work<W::Next>;

    type Error = W::Error;

    async fn next<C: Context<'c>>(self, ctx: &C) -> Result<Self::Next, Self::Error> {
        Ok(Work::<W::Next> {
            project_name: self.project_name,
            account_name: self.account_name,
            work: self.work.next(ctx).await?,
        })
    }
}

impl<'c, W> EndState<'c> for Work<W>
where
    W: EndState<'c>,
{
    type ErrorVariant = W::ErrorVariant;

    fn is_done(&self) -> bool {
        self.work.is_done()
    }

    fn into_result(self) -> Result<Self, Self::ErrorVariant> {
        Ok(Self {
            project_name: self.project_name,
            account_name: self.account_name,
            work: self.work.into_result()?,
        })
    }
}

pub struct Worker<Svc, W> {
    pub service: Svc,
    send: Option<Sender<W>>,
    recv: Receiver<W>,
}

impl<Svc, W> Worker<Svc, W>
where
    W: Send,
{
    pub fn new(service: Svc) -> Self {
        let (send, recv) = channel(256);
        Self {
            service,
            send: Some(send),
            recv,
        }
    }
}

impl<Svc, W> Worker<Svc, W> {
    /// # Panics
    /// If this worker has already been started before.
    pub fn sender(&self) -> Sender<W> {
        self.send.as_ref().unwrap().clone()
    }
}

impl<Svc, W> Worker<Svc, W>
where
    Svc: for<'c> Service<'c, State = W, Error = Error>,
    W: Debug + Send + for<'c> EndState<'c>,
{
    /// Starts the worker, waiting and processing elements from the
    /// queue until the last sending end for the channel is dropped,
    /// at which point this future resolves.
    pub async fn start(mut self) -> Result<Self, Error> {
        // Drop our sender to prevent a deadlock if this is the last
        // one for this channel
        self.send = None;

        while let Some(mut work) = self.recv.recv().await {
            loop {
                work = {
                    let context = self.service.context();

                    // Safety: EndState's transitions are Infallible
                    work.next(&context).await.unwrap()
                };

                match self.service.update(&work).await {
                    Ok(_) => {}
                    Err(err) => info!("failed to update a state: {}\nstate: {:?}", err, work),
                };

                if work.is_done() {
                    break;
                }
            }
        }
        Ok(self)
    }
}

pub struct GatewayService {
    docker: Docker,
    hyper: HyperClient<HttpConnector, Body>,
    db: SqlitePool,
    sender: Mutex<Option<Sender<Work>>>,
    args: Args,
}

use crate::auth::User;
use crate::{auth::Key, AccountName};

impl GatewayService {
    /// Initialize `GatewayService` and its required dependencies.
    ///
    /// * `args` - The [`Args`] with which the service was
    /// started. Will be passed as [`Context`] to workers and state.
    pub async fn init(args: Args) -> Arc<Self> {
        let docker = Docker::connect_with_local_defaults().unwrap();

        let db_uri = &args.state;

        let hyper = HyperClient::new();
        if !StdPath::new(db_uri).exists() {
            Sqlite::create_database(db_uri).await.unwrap();
        }

        info!(
            "state db: {}",
            std::fs::canonicalize(db_uri).unwrap().to_string_lossy()
        );
        let db = SqlitePool::connect(db_uri).await.unwrap();

        query("CREATE TABLE IF NOT EXISTS projects (project_name TEXT PRIMARY KEY, account_name TEXT NOT NULL, initial_key TEXT NOT NULL, project_state JSON NOT NULL)")
            .execute(&db)
            .await
            .unwrap();

        query("CREATE TABLE IF NOT EXISTS accounts (account_name TEXT PRIMARY KEY, key TEXT UNIQUE, super_user BOOLEAN DEFAULT FALSE)")
            .execute(&db)
            .await
            .unwrap();

        let sender = Mutex::new(None);

        let service = Arc::new(Self {
            docker,
            hyper,
            db,
            sender,
            args,
        });

        let worker = Worker::new(Arc::clone(&service));
        let sender = worker.sender();
        service.set_sender(Some(sender)).await;
        tokio::spawn({
            let service = Arc::clone(&service);
            async move {
                match worker.start().await {
                    Ok(_) => info!("worker terminated successfully"),
                    Err(err) => error!("worker error: {}", err),
                };
                service.set_sender(None).await;
            }
        });

        // Queue up all the projects for reconciliation
        for Work {
            project_name,
            account_name,
            work,
        } in service
            .iter_projects()
            .await
            .expect("could not list projects")
        {
            match work.refresh(&service.context()).await {
                Ok(work) => service
                    .send(
                        project_name,
                        account_name,
                        work,
                    )
                    .await
                    .expect("failed to queue work at startup"),
                Err(err) => error!("could not refresh state for user=`{account_name}` project=`{project_name}`: {}. Skipping it for now.", err)
            }
        }

        service
    }

    pub async fn set_sender(&self, sender: Option<Sender<Work>>) -> Result<(), Error> {
        *self.sender.lock().await = sender;
        Ok(())
    }

    pub async fn send(
        &self,
        project_name: ProjectName,
        account_name: AccountName,
        work: Project,
    ) -> Result<(), Error> {
        let work = Work {
            project_name,
            account_name,
            work,
        };

        if let Some(sender) = self.sender.lock().await.as_ref() {
            Ok(sender
                .send(work)
                .await
                .or_else(|_| Err(ErrorKind::Internal))?)
        } else {
            Err(Error::from_kind(ErrorKind::Internal))
        }
    }

    pub async fn route(
        &self,
        project_name: &ProjectName,
        mut route: String,
        mut req: Request<Body>,
    ) -> Result<Response<Body>, Error> {
        let target_ip = self
            .find_project(project_name)
            .await?
            .target_ip()?
            .ok_or_else(|| Error::from_kind(ErrorKind::ProjectNotReady))?;

        let control_key = self.control_key_from_project_name(project_name).await?;

        // TODO I don't understand the API for `headers`: it gives an
        // impl. of `Header` which can only be encoded in something
        // that `Extend<HeaderValue>` but `HeaderMap` only impls
        // `Extend<(HeaderName, HeaderValue)>` (as one would expect),
        // therefore why the ugly hack below.
        {
            use axum::headers::Header;
            let auth_header = Authorization::basic(&control_key, "");
            let auth_header_name = Authorization::<Basic>::name();
            let mut auth = vec![];
            auth_header.encode(&mut auth);
            let headers = req.headers_mut();
            headers.remove(auth_header_name);
            headers.append(auth_header_name, auth.pop().unwrap());
        }

        if !route.starts_with("/") {
            route = format!("/{route}");
        }

        route = format!("/projects/{project_name}{route}");

        *req.uri_mut() = route.parse().unwrap();

        let target_url = format!("http://{target_ip}:8001");

        debug!("routing control: {target_url}");

        let resp = hyper_reverse_proxy::call("127.0.0.1".parse().unwrap(), &target_url, req)
            .await
            .map_err(|_| Error::from_kind(ErrorKind::ProjectUnavailable))?;

        Ok(resp)
    }

    async fn iter_projects(&self) -> Result<impl Iterator<Item = Work>, Error> {
        let iter = query("SELECT * FROM projects")
            .fetch_all(&self.db)
            .await?
            .into_iter()
            .map(|row| Work {
                project_name: row.get("project_name"),
                work: row.get::<SqlxJson<Project>, _>("project_state").0,
                account_name: row.get("account_name"),
            });
        Ok(iter)
    }

    pub async fn find_project(&self, project_name: &ProjectName) -> Result<Project, Error> {
        query("SELECT project_state FROM projects WHERE project_name=?1")
            .bind(project_name)
            .fetch_optional(&self.db)
            .await?
            .map(|r| {
                r.try_get::<SqlxJson<Project>, _>("project_state")
                    .unwrap()
                    .0
            })
            .ok_or_else(|| Error::from_kind(ErrorKind::ProjectNotFound))
    }

    async fn update_project(
        &self,
        project_name: &ProjectName,
        project: &Project,
    ) -> Result<(), Error> {
        query("UPDATE projects SET project_state = ?1 WHERE project_name = ?2")
            .bind(&SqlxJson(project))
            .bind(project_name)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn key_from_account_name(&self, account_name: &AccountName) -> Result<Key, Error> {
        let key = query("SELECT key FROM accounts WHERE account_name = ?1")
            .bind(account_name)
            .fetch_optional(&self.db)
            .await?
            .map(|row| row.try_get("key").unwrap())
            .ok_or_else(|| Error::from(ErrorKind::UserNotFound))?;
        Ok(key)
    }

    pub async fn account_name_from_key(&self, key: &Key) -> Result<AccountName, Error> {
        let name = query("SELECT account_name FROM accounts WHERE key = ?1")
            .bind(key)
            .fetch_optional(&self.db)
            .await?
            .map(|row| row.try_get("account_name").unwrap())
            .ok_or_else(|| Error::from(ErrorKind::UserNotFound))?;
        Ok(name)
    }

    pub async fn control_key_from_project_name(
        &self,
        project_name: &ProjectName,
    ) -> Result<String, Error> {
        let control_key = query("SELECT initial_key FROM projects WHERE project_name = ?1")
            .bind(project_name)
            .fetch_optional(&self.db)
            .await?
            .map(|row| row.try_get("initial_key").unwrap())
            .ok_or_else(|| Error::from(ErrorKind::ProjectNotFound))?;
        Ok(control_key)
    }

    pub async fn user_from_account_name(&self, name: AccountName) -> Result<User, Error> {
        let key = self.key_from_account_name(&name).await?;
        let projects = self.iter_user_projects(&name).await?.collect();
        Ok(User {
            name,
            key,
            projects,
        })
    }

    pub async fn user_from_key(&self, key: Key) -> Result<User, Error> {
        let name = self.account_name_from_key(&key).await?;
        let projects = self.iter_user_projects(&name).await?.collect();
        Ok(User {
            name,
            key,
            projects,
        })
    }

    pub async fn create_user(&self, name: AccountName) -> Result<User, Error> {
        let key = Key::new_random();
        query("INSERT INTO accounts (account_name, key) VALUES (?1, ?2)")
            .bind(&name)
            .bind(&key)
            .execute(&self.db)
            .await
            .or_else(|err| {
                // If the error is a broken PK constraint, this is a
                // project name clash
                if let Some(db_err) = err.as_database_error() {
                    if db_err.code().unwrap() == "1555" {
                        // SQLITE_CONSTRAINT_PRIMARYKEY
                        return Err(Error::from_kind(ErrorKind::UserAlreadyExists));
                    }
                }
                // Otherwise this is internal
                return Err(err.into());
            })?;
        Ok(User {
            name,
            key,
            projects: Vec::default(),
        })
    }

    pub async fn is_super_user(&self, account_name: &AccountName) -> Result<bool, Error> {
        let is_super_user = query("SELECT super_user FROM accounts WHERE account_name = ?1")
            .bind(account_name)
            .fetch_optional(&self.db)
            .await?
            .map(|row| row.try_get("super_user").unwrap())
            .unwrap_or(false); // defaults to `false` (i.e. not super user)
        Ok(is_super_user)
    }

    async fn iter_user_projects(
        &self,
        AccountName(account_name): &AccountName,
    ) -> Result<impl Iterator<Item = ProjectName>, Error> {
        let iter = query("SELECT project_name FROM projects WHERE account_name = ?1")
            .bind(account_name)
            .fetch_all(&self.db)
            .await?
            .into_iter()
            .map(|row| row.try_get::<ProjectName, _>("project_name").unwrap());
        Ok(iter)
    }

    pub async fn create_project(
        &self,
        project_name: ProjectName,
        account_name: AccountName,
    ) -> Result<Project, Error> {
        let initial_key = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);

        let project = SqlxJson(Project::Creating(project::ProjectCreating::new(
            project_name.clone(),
            self.args.prefix.clone(),
            initial_key.clone(),
        )));

        query("INSERT INTO projects (project_name, account_name, initial_key, project_state) VALUES (?1, ?2, ?3, ?4)")
            .bind(&project_name)
            .bind(&account_name)
            .bind(&initial_key)
            .bind(&project)
            .execute(&self.db)
            .await
            .or_else(|err| {
                // If the error is a broken PK constraint, this is a
                // project name clash
                if let Some(db_err) = err.as_database_error() {
                    if db_err.code().unwrap() == "1555" {  // SQLITE_CONSTRAINT_PRIMARYKEY
                        return Err(Error::from_kind(ErrorKind::ProjectAlreadyExists))
                    }
                }
                // Otherwise this is internal
                return Err(err.into())
            })?;

        let project = project.0;

        self.send(project_name, account_name, project.clone())
            .await?;

        Ok(project)
    }

    pub async fn destroy_project(
        &self,
        project_name: ProjectName,
        account_name: AccountName,
    ) -> Result<(), Error> {
        let project = self.find_project(&project_name).await?.destroy()?;

        self.send(project_name, account_name, project).await?;

        Ok(())
    }

    fn context<'c>(&'c self) -> GatewayContext<'c> {
        GatewayContext {
            docker: &self.docker,
            hyper: &self.hyper,
            args: &self.args,
        }
    }
}

#[async_trait]
impl<'c> Service<'c> for Arc<GatewayService> {
    type Context = GatewayContext<'c>;

    type State = Work<Project>;

    type Error = Error;

    fn context(&'c self) -> Self::Context {
        GatewayService::context(self)
    }

    async fn update(
        &mut self,
        Work {
            project_name, work, ..
        }: &Self::State,
    ) -> Result<(), Self::Error> {
        self.update_project(project_name, work).await
    }
}

pub struct GatewayContext<'c> {
    docker: &'c Docker,
    hyper: &'c HyperClient<HttpConnector, Body>,
    args: &'c Args,
}

impl<'c> Context<'c> for GatewayContext<'c> {
    fn docker(&self) -> &'c Docker {
        self.docker
    }

    fn args(&self) -> &'c Args {
        self.args
    }
}

#[cfg(test)]
pub mod tests {
    use anyhow::anyhow;

    use std::{convert::Infallible, marker::PhantomData, time::Duration};

    use crate::{
        assert_err_kind,
        tests::{World, WorldContext},
    };

    use super::*;

    pub struct DummyService<S> {
        world: World,
        state: Option<S>,
    }

    impl DummyService<()> {
        pub async fn new<S>() -> anyhow::Result<DummyService<S>> {
            World::new()
                .await
                .map(|world| DummyService { world, state: None })
        }
    }

    #[async_trait]
    impl<'c, S> Service<'c> for DummyService<S>
    where
        S: EndState<'c> + Sync,
    {
        type Context = WorldContext<'c>;

        type State = S;

        type Error = Error;

        fn context(&'c self) -> Self::Context {
            self.world.context()
        }

        async fn update(&mut self, state: &Self::State) -> Result<(), Self::Error> {
            self.state = Some(state.clone());
            Ok(())
        }
    }

    #[derive(Debug, PartialEq, Clone)]
    pub struct FiniteState {
        count: usize,
        max_count: usize,
    }

    #[async_trait]
    impl<'c> State<'c> for FiniteState {
        type Next = Self;

        type Error = Infallible;

        async fn next<C: Context<'c>>(mut self, ctx: &C) -> Result<Self::Next, Self::Error> {
            if self.count < self.max_count {
                self.count += 1;
            }
            Ok(self)
        }
    }

    impl<'c> EndState<'c> for FiniteState {
        type ErrorVariant = anyhow::Error;

        fn is_done(&self) -> bool {
            self.count == self.max_count
        }

        fn into_result(self) -> Result<Self, Self::ErrorVariant> {
            if self.count > self.max_count {
                Err(anyhow!(
                    "count is over max_count: {} > {}",
                    self.count,
                    self.max_count
                ))
            } else {
                Ok(self)
            }
        }
    }

    #[tokio::test]
    async fn worker_queue_and_proceed_until_done() {
        let svc = DummyService::new::<FiniteState>().await.unwrap();

        let worker = Worker::new(svc);

        {
            let sender = worker.sender();

            let state = FiniteState {
                count: 0,
                max_count: 42,
            };

            sender.send(state).await.unwrap();
        }

        let Worker {
            service: DummyService { state, .. },
            ..
        } = worker.start().await.unwrap();

        assert_eq!(
            state,
            Some(FiniteState {
                count: 42,
                max_count: 42
            })
        )
    }

    #[tokio::test]
    async fn service_create_find_user() -> anyhow::Result<()> {
        let world = World::new().await?;
        let svc = GatewayService::init(world.context().args.clone()).await;

        let account_name: AccountName = "test_user_123".parse()?;

        assert_err_kind!(
            svc.user_from_account_name(account_name.clone()).await,
            ErrorKind::UserNotFound
        );

        assert_err_kind!(
            svc.user_from_key(Key("123".to_string())).await,
            ErrorKind::UserNotFound
        );

        let user = svc.create_user(account_name.clone()).await?;

        assert_eq!(
            svc.user_from_account_name(account_name.clone()).await?,
            user
        );

        assert!(!svc.is_super_user(&account_name).await?);

        let User {
            name,
            key,
            projects,
        } = user;

        assert!(projects.is_empty());

        assert_eq!(name, account_name);

        assert_err_kind!(
            svc.create_user(account_name.clone()).await,
            ErrorKind::UserAlreadyExists
        );

        let user_key = svc.key_from_account_name(&account_name).await?;

        assert_eq!(key, user_key);

        Ok(())
    }

    #[tokio::test]
    async fn service_create_find_destroy_project() {
        todo!()
    }
}

use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Body;
use axum::headers::{Authorization, HeaderMapExt};
use axum::http::Request;
use axum::response::Response;
use bollard::network::ListNetworksOptions;
use bollard::{Docker, API_DEFAULT_VERSION};
use hyper::client::connect::dns::GaiResolver;
use hyper::client::HttpConnector;
use hyper::Client;
use hyper_reverse_proxy::ReverseProxy;
use once_cell::sync::Lazy;
use opentelemetry::global;
use opentelemetry_http::HeaderInjector;
use rand::distributions::{Alphanumeric, DistString};
use sqlx::error::DatabaseError;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqlitePool;
use sqlx::types::Json as SqlxJson;
use sqlx::{query, Error as SqlxError, Row};
use tracing::{debug, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::args::ContextArgs;
use crate::auth::{Key, User};
use crate::project::{self, Project};
use crate::worker::Work;
use crate::{AccountName, Context, Error, ErrorKind, ProjectName, Service};

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");
static PROXY_CLIENT: Lazy<ReverseProxy<HttpConnector<GaiResolver>>> =
    Lazy::new(|| ReverseProxy::new(Client::new()));

impl From<SqlxError> for Error {
    fn from(err: SqlxError) -> Self {
        debug!("internal SQLx error: {err}");
        Self::source(ErrorKind::Internal, err)
    }
}

pub struct ContainerSettingsBuilder<'d> {
    docker: &'d Docker,
    prefix: Option<String>,
    image: Option<String>,
    provisioner: Option<String>,
    network_name: Option<String>,
    fqdn: Option<String>,
}

impl<'d> ContainerSettingsBuilder<'d> {
    pub fn new(docker: &'d Docker) -> Self {
        Self {
            docker,
            prefix: None,
            image: None,
            provisioner: None,
            network_name: None,
            fqdn: None,
        }
    }

    pub async fn from_args(self, args: &ContextArgs) -> ContainerSettings {
        let ContextArgs {
            prefix,
            network_name,
            provisioner_host,
            image,
            proxy_fqdn,
            ..
        } = args;
        self.prefix(prefix)
            .image(image)
            .provisioner_host(provisioner_host)
            .network_name(network_name)
            .fqdn(proxy_fqdn)
            .build()
            .await
    }

    pub fn prefix<S: ToString>(mut self, prefix: S) -> Self {
        self.prefix = Some(prefix.to_string());
        self
    }

    pub fn image<S: ToString>(mut self, image: S) -> Self {
        self.image = Some(image.to_string());
        self
    }

    pub fn provisioner_host<S: ToString>(mut self, host: S) -> Self {
        self.provisioner = Some(host.to_string());
        self
    }

    pub fn network_name<S: ToString>(mut self, name: S) -> Self {
        self.network_name = Some(name.to_string());
        self
    }

    pub fn fqdn<S: ToString>(mut self, fqdn: S) -> Self {
        self.fqdn = Some(fqdn.to_string().trim_end_matches('.').to_string());
        self
    }

    /// Resolves the Docker network ID for the given network name.
    ///
    /// # Panics
    /// If no such Docker network can be found.
    async fn resolve_network_id(&self, network_name: &str) -> String {
        self.docker
            .list_networks(Some(ListNetworksOptions {
                filters: HashMap::from([("name", vec![network_name])]),
            }))
            .await
            .unwrap()
            .into_iter()
            .find_map(|network| {
                network.name.as_ref().and_then(|name| {
                    if name == network_name {
                        network.id
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_else(|| panic!("cannot find a Docker network with name=`{network_name}`"))
    }

    pub async fn build(mut self) -> ContainerSettings {
        let prefix = self.prefix.take().unwrap();
        let image = self.image.take().unwrap();
        let provisioner_host = self.provisioner.take().unwrap();

        let network_name = self.network_name.take().unwrap();
        let network_id = self.resolve_network_id(&network_name).await;
        let fqdn = self.fqdn.take().unwrap();

        ContainerSettings {
            prefix,
            image,
            provisioner_host,
            network_name,
            network_id,
            fqdn,
        }
    }
}

pub struct ContainerSettings {
    pub prefix: String,
    pub image: String,
    pub provisioner_host: String,
    pub network_name: String,
    pub network_id: String,
    pub fqdn: String,
}

impl ContainerSettings {
    pub fn builder(docker: &Docker) -> ContainerSettingsBuilder {
        ContainerSettingsBuilder::new(docker)
    }
}

pub struct GatewayContextProvider {
    docker: Docker,
    settings: ContainerSettings,
}

impl GatewayContextProvider {
    pub fn new(docker: Docker, settings: ContainerSettings) -> Self {
        Self { docker, settings }
    }

    pub fn context(&self) -> GatewayContext {
        GatewayContext {
            docker: &self.docker,
            settings: &self.settings,
        }
    }
}

pub struct GatewayService {
    provider: GatewayContextProvider,
    db: SqlitePool,
}

impl GatewayService {
    /// Initialize `GatewayService` and its required dependencies.
    ///
    /// * `args` - The [`Args`] with which the service was
    /// started. Will be passed as [`Context`] to workers and state.
    pub async fn init(args: ContextArgs, db: SqlitePool) -> Self {
        let docker = Docker::connect_with_unix(&args.docker_host, 60, API_DEFAULT_VERSION).unwrap();

        let container_settings = ContainerSettings::builder(&docker).from_args(&args).await;

        let provider = GatewayContextProvider::new(docker, container_settings);

        Self { provider, db }
    }

    pub async fn route(
        &self,
        project_name: &ProjectName,
        mut req: Request<Body>,
    ) -> Result<Response<Body>, Error> {
        let target_ip = self
            .find_project(project_name)
            .await?
            .target_ip()?
            .ok_or_else(|| Error::from_kind(ErrorKind::ProjectNotReady))?;

        let control_key = self.control_key_from_project_name(project_name).await?;
        let auth_header = Authorization::bearer(&control_key)
            .map_err(|e| Error::source(ErrorKind::KeyMalformed, e))?;
        req.headers_mut().typed_insert(auth_header);

        let target_url = format!("http://{target_ip}:8001");

        debug!(target_url, "routing control");

        let cx = Span::current().context();
        global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&cx, &mut HeaderInjector(req.headers_mut()))
        });

        let resp = PROXY_CLIENT
            .call("127.0.0.1".parse().unwrap(), &target_url, req)
            .await
            .map_err(|_| Error::from_kind(ErrorKind::ProjectUnavailable))?;

        Ok(resp)
    }

    pub async fn iter_projects(&self) -> Result<impl Iterator<Item = Work>, Error> {
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
        let query = match project {
            Project::Destroyed(_) => {
                query("DELETE FROM projects WHERE project_name = ?1").bind(project_name)
            }
            _ => query("UPDATE projects SET project_state = ?1 WHERE project_name = ?2")
                .bind(SqlxJson(project))
                .bind(project_name),
        };

        query.execute(&self.db).await?;
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
        let super_user = self.is_super_user(&name).await?;
        let projects = self.iter_user_projects(&name).await?.collect();
        Ok(User {
            name,
            key,
            projects,
            super_user,
        })
    }

    pub async fn user_from_key(&self, key: Key) -> Result<User, Error> {
        let name = self.account_name_from_key(&key).await?;
        let super_user = self.is_super_user(&name).await?;
        let projects = self.iter_user_projects(&name).await?.collect();
        Ok(User {
            name,
            key,
            projects,
            super_user,
        })
    }

    pub async fn create_user(&self, name: AccountName) -> Result<User, Error> {
        let key = Key::new_random();
        query("INSERT INTO accounts (account_name, key) VALUES (?1, ?2)")
            .bind(&name)
            .bind(&key)
            .execute(&self.db)
            .await
            .map_err(|err| {
                // If the error is a broken PK constraint, this is a
                // project name clash
                if let Some(db_err) = err.as_database_error() {
                    if db_err.code().unwrap() == "1555" {
                        // SQLITE_CONSTRAINT_PRIMARYKEY
                        return Error::from_kind(ErrorKind::UserAlreadyExists);
                    }
                }
                // Otherwise this is internal
                err.into()
            })?;
        Ok(User {
            name,
            key,
            projects: Vec::default(),
            super_user: false,
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

    pub async fn set_super_user(
        &self,
        account_name: &AccountName,
        value: bool,
    ) -> Result<(), Error> {
        query("UPDATE accounts SET super_user = ?1 WHERE account_name = ?2")
            .bind(value)
            .bind(account_name)
            .execute(&self.db)
            .await?;
        Ok(())
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
    ) -> Result<Work, Error> {
        let initial_key = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);

        let project = SqlxJson(Project::Creating(project::ProjectCreating::new(
            project_name.clone(),
            initial_key.clone(),
        )));

        query("INSERT INTO projects (project_name, account_name, initial_key, project_state) VALUES (?1, ?2, ?3, ?4)")
            .bind(&project_name)
            .bind(&account_name)
            .bind(&initial_key)
            .bind(&project)
            .execute(&self.db)
            .await
            .map_err(|err| {
                // If the error is a broken PK constraint, this is a
                // project name clash
                if let Some(db_err_code) = err.as_database_error().and_then(DatabaseError::code) {
                    if db_err_code == "1555" {  // SQLITE_CONSTRAINT_PRIMARYKEY
                        return Error::from_kind(ErrorKind::ProjectAlreadyExists)
                    }
                }
                // Otherwise this is internal
                err.into()
            })?;

        let project = project.0;

        Ok(Work {
            project_name,
            account_name,
            work: project,
        })
    }

    pub async fn destroy_project(
        &self,
        project_name: ProjectName,
        account_name: AccountName,
    ) -> Result<Work, Error> {
        let project = self.find_project(&project_name).await?.destroy()?;

        Ok(Work {
            project_name,
            account_name,
            work: project,
        })
    }

    pub fn context(&self) -> GatewayContext {
        self.provider.context()
    }
}

#[async_trait]
impl<'c> Service<'c> for GatewayService {
    type Context = GatewayContext<'c>;

    type State = Work<Project>;

    type Error = Error;

    fn context(&'c self) -> Self::Context {
        GatewayService::context(self)
    }

    async fn update(
        &self,
        Work {
            project_name, work, ..
        }: &Self::State,
    ) -> Result<(), Self::Error> {
        self.update_project(project_name, work).await
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
        &self,
        Work {
            project_name, work, ..
        }: &Self::State,
    ) -> Result<(), Self::Error> {
        self.update_project(project_name, work).await
    }
}

pub struct GatewayContext<'c> {
    docker: &'c Docker,
    settings: &'c ContainerSettings,
}

impl<'c> Context<'c> for GatewayContext<'c> {
    fn docker(&self) -> &'c Docker {
        self.docker
    }

    fn container_settings(&self) -> &'c ContainerSettings {
        self.settings
    }
}

#[cfg(test)]
pub mod tests {

    use std::str::FromStr;

    use super::*;
    use crate::tests::{assert_err_kind, World};

    #[tokio::test]
    async fn service_create_find_user() -> anyhow::Result<()> {
        let world = World::new().await;
        let svc = GatewayService::init(world.args(), world.pool()).await;

        let account_name: AccountName = "test_user_123".parse()?;

        assert_err_kind!(
            svc.user_from_account_name(account_name.clone()).await,
            ErrorKind::UserNotFound
        );

        assert_err_kind!(
            svc.user_from_key(Key::from_str("123").unwrap()).await,
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
            super_user,
        } = user;

        assert!(projects.is_empty());

        assert!(!super_user);

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
    async fn service_create_find_delete_project() -> anyhow::Result<()> {
        let world = World::new().await;
        let svc = Arc::new(GatewayService::init(world.args(), world.pool()).await);

        let neo: AccountName = "neo".parse().unwrap();
        let matrix: ProjectName = "matrix".parse().unwrap();

        let creating_same_project_name = |project: &Project, project_name: &ProjectName| {
            matches!(
                project,
                Project::Creating(creating) if creating.project_name() == project_name
            )
        };

        svc.create_user(neo.clone()).await.unwrap();

        let work = svc
            .create_project(matrix.clone(), neo.clone())
            .await
            .unwrap();

        // work work work work
        let project = work.work;

        assert!(creating_same_project_name(&project, &matrix));

        assert_eq!(svc.find_project(&matrix).await.unwrap(), project);

        let work = svc.destroy_project(matrix.clone(), neo).await.unwrap();

        let project = work.work;

        assert!(matches!(&project, Project::Destroyed(_)));

        svc.update_project(&matrix, &project).await.unwrap();

        assert!(matches!(
            svc.find_project(&matrix).await,
            Err(Error {
                kind: ErrorKind::ProjectNotFound,
                ..
            })
        ));

        Ok(())
    }
}

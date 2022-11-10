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
use sqlx::error::DatabaseError;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqlitePool;
use sqlx::types::Json as SqlxJson;
use sqlx::{query, Error as SqlxError, Row};
use tracing::{debug, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::args::ContextArgs;
use crate::auth::{Key, Permissions, User};
use crate::custom_domain::CustomDomain;
use crate::project::Project;
use crate::task::TaskBuilder;
use crate::{AccountName, DockerContext, Error, ErrorKind, Fqdn, ProjectName};

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

#[derive(Clone)]
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
            docker: self.docker.clone(),
            settings: self.settings.clone(),
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

    pub async fn route_fqdn(&self, req: Request<Body>) -> Result<Response<Body>, Error> {
        let fqdn = req
            .headers()
            .typed_get::<Fqdn>()
            .ok_or_else(|| Error::from(ErrorKind::CustomDomainNotFound))?;
        let project_name = self.project_name_for_custom_domain(&fqdn).await?;

        self.route(&project_name, req).await
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

    pub async fn iter_projects(
        &self,
    ) -> Result<impl Iterator<Item = (ProjectName, AccountName)>, Error> {
        let iter = query("SELECT project_name, account_name FROM projects")
            .fetch_all(&self.db)
            .await?
            .into_iter()
            .map(|row| (row.get("project_name"), row.get("account_name")));
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

    pub async fn update_project(
        &self,
        project_name: &ProjectName,
        project: &Project,
    ) -> Result<(), Error> {
        query("UPDATE projects SET project_state = ?1 WHERE project_name = ?2")
            .bind(SqlxJson(project))
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
        Ok(User::new_with_defaults(name, key))
    }

    pub async fn get_permissions(&self, account_name: &AccountName) -> Result<Permissions, Error> {
        let permissions =
            query("SELECT super_user, account_tier FROM accounts WHERE account_name = ?1")
                .bind(account_name)
                .fetch_optional(&self.db)
                .await?
                .map(|row| {
                    Permissions::builder()
                        .super_user(row.try_get("super_user").unwrap())
                        .tier(row.try_get("account_tier").unwrap())
                        .build()
                })
                .unwrap_or_default(); // defaults to `false` (i.e. not super user)
        Ok(permissions)
    }

    pub async fn set_super_user(
        &self,
        account_name: &AccountName,
        super_user: bool,
    ) -> Result<(), Error> {
        query("UPDATE accounts SET super_user = ?1 WHERE account_name = ?2")
            .bind(super_user)
            .bind(account_name)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn set_permissions(
        &self,
        account_name: &AccountName,
        permissions: &Permissions,
    ) -> Result<(), Error> {
        query("UPDATE accounts SET super_user = ?1, account_tier = ?2 WHERE account_name = ?3")
            .bind(permissions.super_user)
            .bind(permissions.tier)
            .bind(account_name)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn iter_user_projects(
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
        if let Some(row) = query("SELECT project_name, account_name, initial_key, project_state FROM projects WHERE project_name = ?1 AND account_name = ?2")
            .bind(&project_name)
            .bind(&account_name)
            .fetch_optional(&self.db)
            .await?
        {
            // If the project already exists and belongs to this account
            let project = row.get::<SqlxJson<Project>, _>("project_state").0;
            if project.is_destroyed() {
                // But is in `::Destroyed` state, recreate it
                let project = SqlxJson(Project::create(project_name.clone()));
                query("UPDATE projects SET project_state = ?1, initial_key = ?2 WHERE project_name = ?3")
                    .bind(&project)
                    .bind(project.initial_key().unwrap())
                    .bind(&project_name)
                    .execute(&self.db)
                    .await?;
                Ok(project.0)
            } else {
                // Otherwise it already exists
                Err(Error::from_kind(ErrorKind::ProjectAlreadyExists))
            }
        } else {
            // Otherwise attempt to create a new one. This will fail
            // outright if the project already exists (this happens if
            // it belongs to another account).
            self.insert_project(project_name, account_name).await
        }
    }

    pub async fn insert_project(
        &self,
        project_name: ProjectName,
        account_name: AccountName,
    ) -> Result<Project, Error> {
        let project = SqlxJson(Project::create(project_name.clone()));

        query("INSERT INTO projects (project_name, account_name, initial_key, project_state) VALUES (?1, ?2, ?3, ?4)")
            .bind(&project_name)
            .bind(&account_name)
            .bind(project.initial_key().unwrap())
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

        Ok(project)
    }

    pub async fn create_custom_domain(
        &self,
        project_name: ProjectName,
        fqdn: Fqdn,
    ) -> Result<CustomDomain, Error> {
        let state = SqlxJson(CustomDomain::Creating);

        query("INSERT INTO custom_domains (fqdn, project_name, state) VALUES (?1, ?2, ?3)")
            .bind(&fqdn)
            .bind(&project_name)
            .bind(&state)
            .execute(&self.db)
            .await
            .map_err(|err| {
                if let Some(db_err_code) = err.as_database_error().and_then(DatabaseError::code) {
                    if db_err_code == "1555" {
                        return Error::from(ErrorKind::CustomDomainAlreadyExists);
                    }
                }

                err.into()
            })?;

        Ok(state.0)
    }

    pub async fn project_name_for_custom_domain(&self, fqdn: &Fqdn) -> Result<ProjectName, Error> {
        let project_name = query("SELECT project_name FROM custom_domains WHERE fqdn = ?1")
            .bind(fqdn)
            .fetch_optional(&self.db)
            .await?
            .map(|row| row.try_get("project_name").unwrap())
            .ok_or_else(|| Error::from(ErrorKind::CustomDomainNotFound))?;
        Ok(project_name)
    }

    pub fn context(&self) -> GatewayContext {
        self.provider.context()
    }

    /// Create a builder for a new [ProjectTask]
    pub fn new_task(self: &Arc<Self>) -> TaskBuilder {
        TaskBuilder::new(self.clone())
    }
}

#[derive(Clone)]
pub struct GatewayContext {
    docker: Docker,
    settings: ContainerSettings,
}

impl DockerContext for GatewayContext {
    fn docker(&self) -> &Docker {
        &self.docker
    }

    fn container_settings(&self) -> &ContainerSettings {
        &self.settings
    }
}

#[cfg(test)]
pub mod tests {

    use std::str::FromStr;

    use super::*;
    use crate::auth::AccountTier;
    use crate::task::{self, TaskResult};
    use crate::tests::{assert_err_kind, World};
    use crate::{Error, ErrorKind};

    #[tokio::test]
    async fn service_create_find_user() -> anyhow::Result<()> {
        let world = World::new().await;
        let svc = GatewayService::init(world.args(), world.pool()).await;

        let account_name: AccountName = "test_user_123".parse()?;

        assert_err_kind!(
            User::retrieve_from_account_name(&svc, account_name.clone()).await,
            ErrorKind::UserNotFound
        );

        assert_err_kind!(
            User::retrieve_from_key(&svc, Key::from_str("123").unwrap()).await,
            ErrorKind::UserNotFound
        );

        let user = svc.create_user(account_name.clone()).await?;

        assert_eq!(
            User::retrieve_from_account_name(&svc, account_name.clone()).await?,
            user
        );

        let User {
            name,
            key,
            projects,
            permissions,
        } = user;

        assert!(projects.is_empty());

        assert!(!permissions.is_super_user());

        assert_eq!(*permissions.tier(), AccountTier::Basic);

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
        let trinity: AccountName = "trinity".parse().unwrap();
        let matrix: ProjectName = "matrix".parse().unwrap();

        let creating_same_project_name = |project: &Project, project_name: &ProjectName| {
            matches!(
                project,
                Project::Creating(creating) if creating.project_name() == project_name
            )
        };

        svc.create_user(neo.clone()).await.unwrap();
        svc.create_user(trinity.clone()).await.unwrap();

        let project = svc
            .create_project(matrix.clone(), neo.clone())
            .await
            .unwrap();

        assert!(creating_same_project_name(&project, &matrix));

        assert_eq!(svc.find_project(&matrix).await.unwrap(), project);

        let mut work = svc
            .new_task()
            .project(matrix.clone())
            .account(neo.clone())
            .and_then(task::destroy())
            .build();

        while let TaskResult::Pending(_) = work.poll(()).await {}
        assert!(matches!(work.poll(()).await, TaskResult::Done(())));

        // After project has been destroyed...
        assert!(matches!(
            svc.find_project(&matrix).await,
            Ok(Project::Destroyed(_))
        ));

        // If recreated by a different user
        assert!(matches!(
            svc.create_project(matrix.clone(), trinity.clone()).await,
            Err(Error {
                kind: ErrorKind::ProjectAlreadyExists,
                ..
            })
        ));

        // If recreated by the same user
        assert!(matches!(
            svc.create_project(matrix, neo).await,
            Ok(Project::Creating(_))
        ));

        Ok(())
    }

    #[tokio::test]
    async fn service_create_ready_kill_restart_docker() -> anyhow::Result<()> {
        let world = World::new().await;
        let svc = Arc::new(GatewayService::init(world.args(), world.pool()).await);

        let neo: AccountName = "neo".parse().unwrap();
        let matrix: ProjectName = "matrix".parse().unwrap();

        svc.create_user(neo.clone()).await.unwrap();
        svc.create_project(matrix.clone(), neo.clone())
            .await
            .unwrap();

        let mut task = svc
            .new_task()
            .account(neo.clone())
            .project(matrix.clone())
            .build();

        while let TaskResult::Pending(_) = task.poll(()).await {
            // keep polling
        }

        let project = svc.find_project(&matrix).await.unwrap();
        println!("{:?}", project);
        assert!(project.is_ready());

        let container = project.container().unwrap();
        svc.context()
            .docker()
            .kill_container::<String>(container.name.unwrap().strip_prefix('/').unwrap(), None)
            .await
            .unwrap();

        println!("killed container");

        let mut ambulance_task = svc
            .new_task()
            .project(matrix.clone())
            .account(neo.clone())
            .and_then(task::check_health())
            .build();

        // the first poll will trigger a refresh
        let _ = ambulance_task.poll(()).await;

        let project = svc.find_project(&matrix).await.unwrap();
        println!("{:?}", project);
        assert!(!project.is_ready());

        // the subsequent will trigger a restart task
        while let TaskResult::Pending(_) = ambulance_task.poll(()).await {
            // keep polling
        }

        let project = svc.find_project(&matrix).await.unwrap();
        println!("{:?}", project);
        assert!(project.is_ready());

        Ok(())
    }

    #[tokio::test]
    async fn service_create_find_custom_domain() -> anyhow::Result<()> {
        let world = World::new().await;
        let svc = Arc::new(GatewayService::init(world.args(), world.fqdn(), world.pool()).await);

        let account: AccountName = "neo".parse().unwrap();
        let project_name: ProjectName = "matrix".parse().unwrap();
        let domain: Fqdn = "neo.the.matrix".parse().unwrap();

        svc.create_user(account.clone()).await.unwrap();

        assert_err_kind!(
            svc.project_name_for_custom_domain(&domain).await,
            ErrorKind::CustomDomainNotFound
        );

        let _ = svc
            .create_project(project_name.clone(), account.clone())
            .await
            .unwrap();

        svc.create_custom_domain(project_name.clone(), domain.clone())
            .await
            .unwrap();

        let project = svc.project_name_for_custom_domain(&domain).await.unwrap();

        assert_eq!(project, project_name);

        assert_err_kind!(
            svc.create_custom_domain(project_name.clone(), domain.clone())
                .await,
            ErrorKind::CustomDomainAlreadyExists
        );

        Ok(())
    }
}

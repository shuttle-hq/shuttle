use std::net::SocketAddr;
use std::path::Path;
use std::str::FromStr;

use chrono::Utc;
use error::{Error, Result};
use hyper::Uri;
use shuttle_common::backends::client::gateway::Client as GatewayClient;
use shuttle_common::persistence::deployment::AddressGetter;
use shuttle_common::{
    claims::{Claim, ClaimLayer, InjectPropagationLayer},
    persistence::{
        deployment::{ActiveDeploymentsGetter, Deployment, DeploymentRunnable, DeploymentUpdater},
        service::Service,
        state::{DeploymentState, State, StateRecorder},
        DeployerPersistenceApi,
    },
    resource::Type,
};
use shuttle_proto::{
    provisioner::{provisioner_client::ProvisionerClient, DatabaseRequest},
    resource_recorder::{
        self, record_request, RecordRequest, ResourceIds, ResourceResponse, ResourcesResponse,
        ResultResponse, ServiceResourcesRequest,
    },
};
use sqlx::QueryBuilder;
use sqlx::{
    migrate::{MigrateDatabase, Migrator},
    sqlite::{Sqlite, SqliteConnectOptions, SqliteJournalMode, SqlitePool},
};
use tokio::task::JoinHandle;
use tonic::{transport::Endpoint, Request};
use tower::ServiceBuilder;
use tracing::{error, info, instrument, trace};
use ulid::Ulid;
use uuid::Uuid;

mod error;
pub mod resource;
mod user;

pub use self::error::Error as PersistenceError;
use self::resource::{Resource, ResourceManager};
pub use self::user::User;

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
pub enum Mode {
    Local(SqlitePool),
    Remote(GatewayClient),
}

#[async_trait::async_trait]
impl DeploymentUpdater for Mode {
    type Err = Error;

    /// Set the address for a deployment
    async fn set_address(&self, id: &Uuid, address: &SocketAddr) -> Result<()> {
        match self {
            Mode::Local(pool) => sqlx::query("UPDATE deployments SET address = ? WHERE id = ?")
                .bind(address.to_string())
                .bind(id)
                .execute(pool)
                .await
                .map(|_| ())
                .map_err(Self::Err::from),
            Mode::Remote(client) => client.set_address(id, address).await.map_err(Error::from),
        }
    }

    /// Set if a deployment is build on shuttle-next
    async fn set_is_next(&self, id: &Uuid, is_next: bool) -> Result<()> {
        match self {
            Mode::Local(pool) => sqlx::query("UPDATE deployments SET is_next = ? WHERE id = ?")
                .bind(is_next)
                .bind(id)
                .execute(pool)
                .await
                .map(|_| ())
                .map_err(Self::Err::from),
            Mode::Remote(client) => client.set_is_next(id, is_next).await.map_err(Error::from),
        }
    }

    /// Update the state
    async fn set_state(&self, state: DeploymentState) -> Result<()> {
        match self {
            Mode::Local(pool) => {
                sqlx::query("UPDATE deployments SET state = ?, last_update = ? WHERE id = ?")
                    .bind(state.state)
                    .bind(Utc::now())
                    .bind(state.id)
                    .execute(pool)
                    .await
                    .map(|_| ())
                    .map_err(Self::Err::from)
            }
            Mode::Remote(client) => client.set_state(state).await.map_err(Error::from),
        }
    }

    async fn update_deployment(&self, state: DeploymentState) -> Result<()> {
        match self {
            Mode::Local(pool) => {
                sqlx::query("UPDATE deployments SET state = ?, last_update = ? WHERE id = ?")
                    .bind(state.state)
                    .bind(Utc::now())
                    .bind(state.id)
                    .execute(pool)
                    .await
                    .map(|_| ())
                    .map_err(Self::Err::from)
            }
            Mode::Remote(client) => client.update_deployment(state).await.map_err(Error::from),
        }
    }
}

#[async_trait::async_trait]
impl AddressGetter for Mode {
    type Err = Error;

    async fn get_address_for_service(
        &self,
        service_name: &str,
    ) -> Result<Option<std::net::SocketAddr>> {
        match self {
            Mode::Local(pool) => {
                let address_str = sqlx::query_as::<_, (String,)>(
                    r#"SELECT d.address
                FROM deployments AS d
                JOIN services AS s ON d.service_id = s.id
                WHERE s.name = ? AND d.state = ?
                ORDER BY d.last_update
                DESC"#,
                )
                .bind(service_name)
                .bind(State::Running)
                .fetch_optional(pool)
                .await
                .map_err(Self::Err::from)?;

                if let Some((address_str,)) = address_str {
                    SocketAddr::from_str(&address_str).map(Some).map_err(|err| {
                        Error::ParseError(format!(
                            "couldn't parse socket address from persistence: {err}"
                        ))
                    })
                } else {
                    Ok(None)
                }
            }
            Mode::Remote(client) => client
                .get_address_for_service(service_name)
                .await
                .map_err(Error::from),
        }
    }
}

#[async_trait::async_trait]
impl ActiveDeploymentsGetter for Mode {
    type Err = Error;

    async fn get_active_deployments(&self, service_id: &Ulid) -> Result<Vec<Uuid>> {
        match self {
            Mode::Local(pool) => {
                let ids: Vec<_> = sqlx::query_as::<_, Deployment>(
                    "SELECT * FROM deployments WHERE service_id = ? AND state = ?",
                )
                .bind(service_id.to_string())
                .bind(State::Running)
                .fetch_all(pool)
                .await
                .map_err(Self::Err::from)?
                .into_iter()
                .map(|deployment| deployment.id)
                .collect();

                Ok(ids)
            }
            Mode::Remote(client) => client
                .get_active_deployments(service_id)
                .await
                .map_err(Error::from),
        }
    }
}

#[async_trait::async_trait]
impl DeployerPersistenceApi for Mode {
    type MasterErr = Error;

    async fn insert_deployment(&self, deployment: impl Into<&Deployment> + Send) -> Result<()> {
        match self {
            Mode::Local(pool) => {
                let deployment: &Deployment = deployment.into();

                sqlx::query("INSERT INTO deployments VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
                    .bind(deployment.id)
                    .bind(deployment.service_id.to_string())
                    .bind(deployment.state)
                    .bind(deployment.last_update)
                    .bind(deployment.address.map(|socket| socket.to_string()))
                    .bind(deployment.is_next)
                    .bind(deployment.git_commit_id.as_ref())
                    .bind(deployment.git_commit_msg.as_ref())
                    .bind(deployment.git_branch.as_ref())
                    .bind(deployment.git_dirty)
                    .execute(pool)
                    .await
                    .map(|_| ())
                    .map_err(Error::from)
            }
            Mode::Remote(client) => client
                .insert_deployment(deployment)
                .await
                .map_err(Error::from),
        }
    }

    async fn get_deployment(&self, id: &Uuid) -> Result<Option<Deployment>> {
        match self {
            Mode::Local(pool) => sqlx::query_as("SELECT * FROM deployments WHERE id = ?")
                .bind(id)
                .fetch_optional(pool)
                .await
                .map_err(Error::from),
            Mode::Remote(client) => client.get_deployment(id).await.map_err(Error::from),
        }
    }

    async fn get_deployments(
        &self,
        service_id: &Ulid,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<Deployment>> {
        match self {
            Mode::Local(pool) => {
                let mut query = QueryBuilder::new("SELECT * FROM deployments WHERE service_id = ");

                query
                    .push_bind(service_id.to_string())
                    .push(" ORDER BY last_update DESC LIMIT ")
                    .push_bind(limit);

                if offset > 0 {
                    query.push(" OFFSET ").push_bind(offset);
                }

                query
                    .build_query_as()
                    .fetch_all(pool)
                    .await
                    .map_err(Error::from)
            }
            Mode::Remote(client) => client
                .get_deployments(service_id, offset, limit)
                .await
                .map_err(Error::from),
        }
    }

    async fn get_active_deployment(&self, service_id: &Ulid) -> Result<Option<Deployment>> {
        match self {
            Mode::Local(pool) => {
                sqlx::query_as("SELECT * FROM deployments WHERE service_id = ? AND state = ?")
                    .bind(service_id.to_string())
                    .bind(State::Running)
                    .fetch_optional(pool)
                    .await
                    .map_err(Error::from)
            }
            Mode::Remote(client) => client
                .get_active_deployment(service_id)
                .await
                .map_err(Error::from),
        }
    }

    async fn cleanup_invalid_states(&self) -> Result<()> {
        match self {
            Mode::Local(pool) => {
                sqlx::query("UPDATE deployments SET state = ? WHERE state IN(?, ?, ?, ?)")
                    .bind(State::Stopped)
                    .bind(State::Queued)
                    .bind(State::Built)
                    .bind(State::Building)
                    .bind(State::Loading)
                    .execute(pool)
                    .await?;

                Ok(())
            }
            Mode::Remote(client) => client.cleanup_invalid_states().await.map_err(Error::from),
        }
    }

    async fn get_service_by_name(&self, name: &str) -> Result<Option<Service>> {
        match self {
            Mode::Local(pool) => sqlx::query_as("SELECT * FROM services WHERE name = ?")
                .bind(name)
                .fetch_optional(pool)
                .await
                .map_err(Error::from),
            Mode::Remote(client) => client.get_service_by_name(name).await.map_err(Error::from),
        }
    }

    async fn get_or_create_service(&self, name: &str) -> Result<Service> {
        match self {
            Mode::Local(pool) => {
                if let Some(service) = self.get_service_by_name(name).await? {
                    Ok(service)
                } else {
                    let service = Service {
                        id: Ulid::new(),
                        name: name.to_string(),
                    };

                    sqlx::query("INSERT INTO services (id, name) VALUES (?, ?)")
                        .bind(service.id.to_string())
                        .bind(&service.name)
                        .execute(pool)
                        .await?;

                    Ok(service)
                }
            }
            Mode::Remote(client) => client
                .get_or_create_service(name)
                .await
                .map_err(Error::from),
        }
    }

    async fn delete_service(&self, id: &Ulid) -> Result<()> {
        match self {
            Mode::Local(pool) => sqlx::query("DELETE FROM services WHERE id = ?")
                .bind(id.to_string())
                .execute(pool)
                .await
                .map(|_| ())
                .map_err(Error::from),
            Mode::Remote(client) => client.delete_service(id).await.map_err(Error::from),
        }
    }

    async fn get_all_services(&self) -> Result<Vec<Service>> {
        match self {
            Mode::Local(pool) => sqlx::query_as("SELECT * FROM services")
                .fetch_all(pool)
                .await
                .map_err(Error::from),
            Mode::Remote(client) => client.get_all_services().await.map_err(Error::from),
        }
    }

    async fn get_all_runnable_deployments(&self) -> Result<Vec<DeploymentRunnable>> {
        match self {
            Mode::Local(pool) => sqlx::query_as(
                r#"SELECT d.id, service_id, s.name AS service_name, d.is_next
                FROM deployments AS d
                JOIN services AS s ON s.id = d.service_id
                WHERE state = ?
                ORDER BY last_update DESC"#,
            )
            .bind(State::Running)
            .fetch_all(pool)
            .await
            .map_err(Error::from),
            Mode::Remote(client) => client
                .get_all_runnable_deployments()
                .await
                .map_err(Error::from),
        }
    }

    async fn get_runnable_deployment(&self, id: &Uuid) -> Result<Option<DeploymentRunnable>> {
        match self {
            Mode::Local(pool) => sqlx::query_as(
                r#"SELECT d.id, service_id, s.name AS service_name, d.is_next
                FROM deployments AS d
                JOIN services AS s ON s.id = d.service_id
                WHERE state IN (?, ?, ?)
                AND d.id = ?"#,
            )
            .bind(State::Running)
            .bind(State::Stopped)
            .bind(State::Completed)
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(Error::from),
            Mode::Remote(client) => client
                .get_runnable_deployment(id)
                .await
                .map_err(Error::from),
        }
    }
}

#[derive(Clone)]
pub struct Persistence {
    mode: Mode,
    state_send: tokio::sync::mpsc::UnboundedSender<DeploymentState>,
    resource_recorder_client: Option<resource_recorder::Client>,
    provisioner_client: Option<
        ProvisionerClient<
            shuttle_common::claims::ClaimService<
                shuttle_common::claims::InjectPropagation<tonic::transport::Channel>,
            >,
        >,
    >,
    project_id: Ulid,
}

impl Persistence {
    /// Creates a persistent storage solution (i.e., SQL database). This
    /// function creates all necessary tables and sets up a database connection
    /// pool - new connections should be made by cloning [`Persistence`] rather
    /// than repeatedly calling [`Persistence::new`].
    pub async fn new(
        state_connection_uri: &str,
        is_remote_persistence: bool,
        resource_recorder_uri: Uri,
        provisioner_address: &Uri,
        project_id: Ulid,
    ) -> (Self, JoinHandle<()>) {
        if is_remote_persistence {
            return Self::configure_remote(
                state_connection_uri,
                resource_recorder_uri,
                provisioner_address.to_string(),
                project_id,
            )
            .await;
        }

        Self::configure_local(
            state_connection_uri,
            resource_recorder_uri,
            provisioner_address.to_string(),
            project_id,
        )
        .await
    }

    #[cfg(test)]
    async fn new_in_memory() -> (Self, JoinHandle<()>) {
        let pool = SqlitePool::connect_with(
            SqliteConnectOptions::from_str("sqlite::memory:")
                .unwrap()
                // Set the ulid0 extension for generating ULID's in migrations.
                // This uses the ulid0.so file in the crate root, with the
                // LD_LIBRARY_PATH env set in build.rs.
                .extension("ulid0"),
        )
        .await
        .unwrap();
        MIGRATIONS.run(&pool).await.unwrap();
        let mode = Mode::Local(pool);
        let (state_send, handle) = Self::state_updater_hook(mode.clone()).await;
        let persistence = Self {
            mode,
            state_send,
            resource_recorder_client: None,
            provisioner_client: None,
            project_id: Ulid::new(),
        };

        (persistence, handle)
    }

    async fn configure_local(
        state_connection_uri: &str,
        resource_recorder_uri: Uri,
        provisioner_address: String,
        project_id: Ulid,
    ) -> (Self, JoinHandle<()>) {
        if !Path::new(state_connection_uri).exists() {
            Sqlite::create_database(state_connection_uri).await.unwrap();
        }

        info!(
            "state db: {}",
            std::fs::canonicalize(state_connection_uri)
                .unwrap()
                .to_string_lossy()
        );

        // We have found in the past that setting synchronous to anything other than the default (full) breaks the
        // broadcast channel in deployer. The broken symptoms are that the ws socket connections won't get any logs
        // from the broadcast channel and would then close. When users did deploys, this would make it seem like the
        // deploy is done (while it is still building for most of the time) and the status of the previous deployment
        // would be returned to the user.
        //
        // If you want to activate a faster synchronous mode, then also do proper testing to confirm this bug is no
        // longer present.
        let sqlite_options = SqliteConnectOptions::from_str(state_connection_uri)
            .unwrap()
            .journal_mode(SqliteJournalMode::Wal)
            // Set the ulid0 extension for converting UUIDs to ULID's in migrations.
            // This uses the ulid0.so file in the crate root, with the
            // LD_LIBRARY_PATH env set in build.rs.
            .extension("ulid0");

        let pool = SqlitePool::connect_with(sqlite_options)
            .await
            .expect("to be able to connect to sqlite with the options");

        MIGRATIONS.run(&pool).await.unwrap();
        let channel = Endpoint::from_shared(provisioner_address)
            .expect("to have a valid string endpoint for the provisioner")
            .connect()
            .await
            .expect("failed to connect to provisioner");

        let provisioner_service = ServiceBuilder::new()
            .layer(ClaimLayer)
            .layer(InjectPropagationLayer)
            .service(channel);

        let resource_recorder_client = resource_recorder::get_client(resource_recorder_uri).await;
        let provisioner_client = ProvisionerClient::new(provisioner_service);
        let mode = Mode::Local(pool);
        let (state_send, handle) = Self::state_updater_hook(mode.clone()).await;

        let persistence = Self {
            mode,
            state_send,
            resource_recorder_client: Some(resource_recorder_client),
            provisioner_client: Some(provisioner_client),
            project_id,
        };

        (persistence, handle)
    }

    async fn configure_remote(
        state_connection_uri: &str,
        resource_recorder_uri: Uri,
        provisioner_address: String,
        project_id: Ulid,
    ) -> (Self, JoinHandle<()>) {
        let gateway_deployer_api_uri =
            Uri::from_str(state_connection_uri).expect("to have a valid gateway deployer API uri");

        // The private and public APIs are the same, but in a next iteration, once all deployers use
        // the remote persistence, we can set a valid public API that can be used to handle current
        // deployer APIs that don't depend on the user space that the deployer manages, and as a result
        // can live outside the deployer, centrally.
        let gateway_client = shuttle_common::backends::client::gateway::Client::new(
            gateway_deployer_api_uri.clone(),
            gateway_deployer_api_uri,
        );

        let channel = Endpoint::from_shared(provisioner_address)
            .expect("to have a valid string endpoint for the provisioner")
            .connect()
            .await
            .expect("failed to connect to provisioner");

        let provisioner_service = ServiceBuilder::new()
            .layer(ClaimLayer)
            .layer(InjectPropagationLayer)
            .service(channel);

        let resource_recorder_client = resource_recorder::get_client(resource_recorder_uri).await;
        let provisioner_client = ProvisionerClient::new(provisioner_service);

        let (state_send, handle) = Self::state_updater_hook(gateway_client.clone()).await;

        let persistence = Self {
            mode: Mode::Remote(gateway_client),
            state_send,
            resource_recorder_client: Some(resource_recorder_client),
            provisioner_client: Some(provisioner_client),
            project_id,
        };

        (persistence, handle)
    }

    async fn state_updater_hook(
        client: impl DeploymentUpdater,
    ) -> (
        tokio::sync::mpsc::UnboundedSender<DeploymentState>,
        JoinHandle<()>,
    ) {
        // Unbounded channel so that sync code (tracing layer) can send to async listener (here)
        let (state_send, mut state_recv) =
            tokio::sync::mpsc::unbounded_channel::<DeploymentState>();

        let handle = tokio::spawn(async move {
            while let Some(state) = state_recv.recv().await {
                trace!(?state, "persistence received state change");
                client.set_state(state).await.unwrap_or_else(|error| {
                    error!(
                        error = &error as &dyn std::error::Error,
                        "failed to update deployment state"
                    )
                });
            }
        });

        (state_send, handle)
    }

    pub fn project_id(&self) -> Ulid {
        self.project_id
    }

    pub async fn get_deployment(&self, id: &Uuid) -> Result<Option<Deployment>> {
        self.mode.get_deployment(id).await
    }

    pub async fn stop_running_deployment(&self, deployable: DeploymentRunnable) -> Result<()> {
        self.mode
            .update_deployment(DeploymentState {
                id: deployable.id,
                state: State::Stopped,
            })
            .await
    }
}

#[async_trait::async_trait]
impl ResourceManager for Persistence {
    type Err = Error;

    async fn insert_resources(
        &mut self,
        resources: Vec<record_request::Resource>,
        service_id: &Ulid,
        claim: Claim,
    ) -> Result<ResultResponse> {
        let mut record_req: tonic::Request<RecordRequest> = tonic::Request::new(RecordRequest {
            project_id: self.project_id.to_string(),
            service_id: service_id.to_string(),
            resources,
        });

        record_req.extensions_mut().insert(claim);

        info!("Uploading resources to resource-recorder");
        self.resource_recorder_client
            .as_mut()
            .expect("to have the resource recorder set up")
            .record_resources(record_req)
            .await
            .map_err(PersistenceError::ResourceRecorder)
            .map(|res| res.into_inner())
    }

    async fn get_resources(
        &mut self,
        service_id: &Ulid,
        claim: Claim,
    ) -> Result<ResourcesResponse> {
        let mut service_resources_req = tonic::Request::new(ServiceResourcesRequest {
            service_id: service_id.to_string(),
        });

        service_resources_req.extensions_mut().insert(claim.clone());

        info!(%service_id, "Getting resources from resource-recorder");
        let res = self
            .resource_recorder_client
            .as_mut()
            .expect("to have the resource recorder set up")
            .get_service_resources(service_resources_req)
            .await
            .map_err(PersistenceError::ResourceRecorder)
            .map(|res| res.into_inner())?;

        // If the resources list is empty and we're using the persistence in local mode
        let mode = self.mode.clone();
        match mode {
            Mode::Remote(_) => Ok(res),
            Mode::Local(pool) => {
                if res.resources.is_empty() {
                    info!("Got no resources from resource-recorder");
                    // Check if there are cached resources on the local persistence.
                    let resources: std::result::Result<Vec<Resource>, sqlx::Error> =
                        sqlx::query_as("SELECT * FROM resources WHERE service_id = ?")
                            .bind(service_id.to_string())
                            .fetch_all(&pool)
                            .await;

                    info!(?resources, "Local resources");
                    // If there are cached resources
                    if let Ok(inner) = resources {
                        // Return early if the local persistence is empty.
                        if inner.is_empty() {
                            return Ok(res);
                        }

                        // Insert local resources in the resource-recorder.
                        let local_resources = inner
                            .into_iter()
                            .map(|res| record_request::Resource {
                                r#type: res.r#type.to_string(),
                                config: res.config.to_string().into_bytes(),
                                data: res.data.to_string().into_bytes(),
                            })
                            .collect();

                        self.insert_resources(local_resources, service_id, claim.clone())
                            .await?;

                        let mut service_resources_req =
                            tonic::Request::new(ServiceResourcesRequest {
                                service_id: service_id.to_string(),
                            });

                        service_resources_req.extensions_mut().insert(claim);

                        info!("Getting resources from resource-recorder again");
                        let res = self
                            .resource_recorder_client
                            .as_mut()
                            .expect("to have the resource recorder set up")
                            .get_service_resources(service_resources_req)
                            .await
                            .map_err(PersistenceError::ResourceRecorder)
                            .map(|res| res.into_inner())?;

                        if res.resources.is_empty() {
                            // Something went wrong since it was empty before the upload, and is still empty now.
                            return Err(Error::ResourceRecorderSync);
                        }

                        info!("Deleting local resources");
                        // Now that we know that the resources are in resource-recorder,
                        // we can safely delete them from here to prevent de-sync issues and to not hinder project deletion
                        sqlx::query("DELETE FROM resources WHERE service_id = ?")
                            .bind(service_id.to_string())
                            .execute(&pool)
                            .await?;

                        return Ok(res);
                    }
                }

                Ok(res)
            }
        }
    }

    async fn get_resource(
        &mut self,
        service_id: &Ulid,
        r#type: shuttle_common::resource::Type,
        claim: Claim,
    ) -> Result<ResourceResponse> {
        let mut get_resource_req = tonic::Request::new(ResourceIds {
            project_id: self.project_id.to_string(),
            service_id: service_id.to_string(),
            r#type: r#type.to_string(),
        });

        get_resource_req.extensions_mut().insert(claim);

        return self
            .resource_recorder_client
            .as_mut()
            .expect("to have the resource recorder set up")
            .get_resource(get_resource_req)
            .await
            .map_err(PersistenceError::ResourceRecorder)
            .map(|res| res.into_inner());
    }

    async fn delete_resource(
        &mut self,
        project_name: String,
        service_id: &Ulid,
        resource_type: shuttle_common::resource::Type,
        claim: Claim,
    ) -> Result<ResultResponse> {
        if let Type::Database(db_type) = resource_type {
            let proto_db_type: shuttle_proto::provisioner::database_request::DbType =
                db_type.into();
            if let Some(inner) = &mut self.provisioner_client {
                let mut db_request = Request::new(DatabaseRequest {
                    project_name,
                    db_type: Some(proto_db_type),
                });
                db_request.extensions_mut().insert(claim.clone());
                inner
                    .delete_database(db_request)
                    .await
                    .map_err(error::Error::Provisioner)?;
            };
        }

        let mut delete_resource_req = tonic::Request::new(ResourceIds {
            project_id: self.project_id.to_string(),
            service_id: service_id.to_string(),
            r#type: resource_type.to_string(),
        });

        delete_resource_req.extensions_mut().insert(claim);

        return self
            .resource_recorder_client
            .as_mut()
            .expect("to have the resource recorder set up")
            .delete_resource(delete_resource_req)
            .await
            .map(|res| res.into_inner())
            .map_err(PersistenceError::ResourceRecorder);
    }
}

#[async_trait::async_trait]
impl AddressGetter for Persistence {
    type Err = Error;

    #[instrument(skip_all, fields(shuttle.service.name = service_name, shuttle.project.name = service_name))]
    async fn get_address_for_service(
        &self,
        service_name: &str,
    ) -> Result<Option<std::net::SocketAddr>> {
        self.mode.get_address_for_service(service_name).await
    }
}

#[async_trait::async_trait]
impl DeploymentUpdater for Persistence {
    type Err = Error;

    async fn set_address(&self, id: &Uuid, address: &SocketAddr) -> Result<()> {
        self.mode.set_address(id, address).await
    }

    async fn set_is_next(&self, id: &Uuid, is_next: bool) -> Result<()> {
        self.mode.set_is_next(id, is_next).await
    }

    async fn set_state(&self, state: DeploymentState) -> Result<()> {
        self.mode.set_state(state).await
    }

    async fn update_deployment(&self, state: DeploymentState) -> Result<()> {
        self.mode.update_deployment(state).await
    }
}

#[async_trait::async_trait]
impl DeployerPersistenceApi for Persistence {
    type MasterErr = Error;

    async fn insert_deployment(&self, deployment: impl Into<&Deployment> + Send) -> Result<()> {
        self.mode.insert_deployment(deployment).await
    }

    async fn get_deployment(&self, id: &Uuid) -> Result<Option<Deployment>> {
        self.mode.get_deployment(id).await
    }

    async fn get_deployments(
        &self,
        service_id: &Ulid,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<Deployment>> {
        self.mode.get_deployments(service_id, offset, limit).await
    }

    async fn get_active_deployment(&self, service_id: &Ulid) -> Result<Option<Deployment>> {
        self.mode.get_active_deployment(service_id).await
    }

    async fn cleanup_invalid_states(&self) -> Result<()> {
        self.mode.cleanup_invalid_states().await
    }

    async fn get_service_by_name(&self, name: &str) -> Result<Option<Service>> {
        self.mode.get_service_by_name(name).await
    }

    async fn get_or_create_service(&self, name: &str) -> Result<Service> {
        self.mode.get_or_create_service(name).await
    }

    async fn delete_service(&self, id: &Ulid) -> Result<()> {
        self.mode.delete_service(id).await
    }

    async fn get_all_services(&self) -> Result<Vec<Service>> {
        self.mode.get_all_services().await
    }

    async fn get_all_runnable_deployments(&self) -> Result<Vec<DeploymentRunnable>> {
        self.mode.get_all_runnable_deployments().await
    }

    async fn get_runnable_deployment(&self, id: &Uuid) -> Result<Option<DeploymentRunnable>> {
        self.mode.get_runnable_deployment(id).await
    }
}

#[async_trait::async_trait]
impl ActiveDeploymentsGetter for Persistence {
    type Err = Error;

    async fn get_active_deployments(
        &self,
        service_id: &Ulid,
    ) -> std::result::Result<Vec<Uuid>, Self::Err> {
        self.mode.get_active_deployments(service_id).await
    }
}

impl StateRecorder for Persistence {
    type Err = Error;

    fn record_state(&self, state: DeploymentState) -> Result<()> {
        self.state_send
            .send(state)
            .map_err(|_| Error::ChannelSendThreadError)
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddr};

    use chrono::{Duration, TimeZone, Utc};
    use rand::Rng;

    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn deployment_updates() {
        let (p, _) = Persistence::new_in_memory().await;
        let Mode::Local(pool) = &p.mode else {
            unreachable!()
        };
        let service_id = add_service(pool).await.unwrap();

        let id = Uuid::new_v4();
        let deployment = Deployment {
            id,
            service_id,
            state: State::Queued,
            last_update: Utc.with_ymd_and_hms(2022, 4, 25, 4, 43, 33).unwrap(),
            ..Default::default()
        };
        let address = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 12345);

        p.insert_deployment(&deployment).await.unwrap();
        assert_eq!(p.get_deployment(&id).await.unwrap().unwrap(), deployment);

        p.update_deployment(DeploymentState {
            id,
            state: State::Built,
        })
        .await
        .unwrap();

        p.set_address(&id, &address).await.unwrap();
        p.set_is_next(&id, true).await.unwrap();

        let update = p.get_deployment(&id).await.unwrap().unwrap();
        assert_eq!(update.state, State::Built);
        assert_eq!(update.address, Some(address));
        assert!(update.is_next);
        assert_ne!(
            update.last_update,
            Utc.with_ymd_and_hms(2022, 4, 25, 4, 43, 33).unwrap()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn get_deployments() {
        let (p, _) = Persistence::new_in_memory().await;
        let Mode::Local(pool) = &p.mode else {
            unreachable!()
        };
        let service_id = add_service(pool).await.unwrap();

        let mut deployments: Vec<_> = (0..10)
            .map(|_| Deployment {
                id: Uuid::new_v4(),
                service_id,
                state: State::Running,
                last_update: Utc::now(),
                address: None,
                is_next: false,
                git_commit_id: None,
                git_commit_msg: None,
                git_branch: None,
                git_dirty: None,
            })
            .collect();

        for deployment in &deployments {
            p.insert_deployment(deployment).await.unwrap();
        }

        // Reverse to match last_updated desc order
        deployments.reverse();
        assert_eq!(
            p.get_deployments(&service_id, 0, 5).await.unwrap(),
            deployments[0..5]
        );
        assert_eq!(
            p.get_deployments(&service_id, 5, 5).await.unwrap(),
            deployments[5..10]
        );
        assert_eq!(p.get_deployments(&service_id, 20, 5).await.unwrap(), vec![]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn deployment_active() {
        let (p, _) = Persistence::new_in_memory().await;
        let Mode::Local(pool) = &p.mode else {
            unreachable!()
        };
        let xyz_id = add_service(pool).await.unwrap();
        let service_id = add_service(pool).await.unwrap();

        let deployment_crashed = Deployment {
            id: Uuid::new_v4(),
            service_id: xyz_id,
            state: State::Crashed,
            last_update: Utc.with_ymd_and_hms(2022, 4, 25, 7, 29, 35).unwrap(),
            address: None,
            is_next: false,
            ..Default::default()
        };
        let deployment_stopped = Deployment {
            id: Uuid::new_v4(),
            service_id: xyz_id,
            state: State::Stopped,
            last_update: Utc.with_ymd_and_hms(2022, 4, 25, 7, 49, 35).unwrap(),
            address: None,
            is_next: false,
            ..Default::default()
        };
        let deployment_other = Deployment {
            id: Uuid::new_v4(),
            service_id,
            state: State::Running,
            last_update: Utc.with_ymd_and_hms(2022, 4, 25, 7, 39, 39).unwrap(),
            address: None,
            is_next: false,
            ..Default::default()
        };
        let deployment_running = Deployment {
            id: Uuid::new_v4(),
            service_id: xyz_id,
            state: State::Running,
            last_update: Utc.with_ymd_and_hms(2022, 4, 25, 7, 48, 29).unwrap(),
            address: Some(SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 9876)),
            is_next: true,
            ..Default::default()
        };

        for deployment in [
            &deployment_crashed,
            &deployment_stopped,
            &deployment_other,
            &deployment_running,
        ] {
            p.insert_deployment(deployment).await.unwrap();
        }

        assert_eq!(
            p.get_active_deployment(&xyz_id).await.unwrap().unwrap(),
            deployment_running
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn deployment_order() {
        let (p, _) = Persistence::new_in_memory().await;

        let Mode::Local(pool) = &p.mode else {
            unreachable!()
        };
        let service_id = add_service(pool).await.unwrap();
        let other_id = add_service(pool).await.unwrap();

        let deployment_other = Deployment {
            id: Uuid::new_v4(),
            service_id: other_id,
            state: State::Running,
            last_update: Utc.with_ymd_and_hms(2023, 4, 17, 1, 1, 2).unwrap(),
            address: None,
            is_next: false,
            ..Default::default()
        };
        let deployment_crashed = Deployment {
            id: Uuid::new_v4(),
            service_id,
            state: State::Crashed,
            last_update: Utc.with_ymd_and_hms(2023, 4, 17, 1, 1, 2).unwrap(), // second
            address: None,
            is_next: false,
            ..Default::default()
        };
        let deployment_stopped = Deployment {
            id: Uuid::new_v4(),
            service_id,
            state: State::Stopped,
            last_update: Utc.with_ymd_and_hms(2023, 4, 17, 1, 1, 1).unwrap(), // first
            address: None,
            is_next: false,
            ..Default::default()
        };
        let deployment_running = Deployment {
            id: Uuid::new_v4(),
            service_id,
            state: State::Running,
            last_update: Utc.with_ymd_and_hms(2023, 4, 17, 1, 1, 3).unwrap(), // third
            address: Some(SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 9876)),
            is_next: true,
            ..Default::default()
        };

        for deployment in [
            &deployment_other,
            &deployment_crashed,
            &deployment_stopped,
            &deployment_running,
        ] {
            p.insert_deployment(deployment).await.unwrap();
        }

        let actual = p.get_deployments(&service_id, 0, u32::MAX).await.unwrap();
        let expected = vec![deployment_running, deployment_crashed, deployment_stopped];

        assert_eq!(actual, expected, "deployments should be sorted by time");
    }

    // Test that we are correctly cleaning up any stale / unexpected states for a deployment
    // The reason this does not clean up two (or more) running states for a single deployment is because
    // it should theoretically be impossible for a service to have two deployments in the running state.
    // And even if a service were to have this, then the start ups of these deployments (more specifically
    // the last deployment that is starting up) will stop all the deployments correctly.
    #[tokio::test(flavor = "multi_thread")]
    async fn cleanup_invalid_states() {
        let (p, _) = Persistence::new_in_memory().await;
        let Mode::Local(pool) = &p.mode else {
            unreachable!()
        };
        let service_id = add_service(pool).await.unwrap();
        let time = Utc::now();

        let deployment_crashed = Deployment {
            id: Uuid::new_v4(),
            service_id,
            state: State::Crashed,
            last_update: time,
            address: None,
            is_next: false,
            ..Default::default()
        };
        let deployment_stopped = Deployment {
            id: Uuid::new_v4(),
            service_id,
            state: State::Stopped,
            last_update: time.checked_add_signed(Duration::seconds(1)).unwrap(),
            address: None,
            is_next: false,
            ..Default::default()
        };
        let deployment_running = Deployment {
            id: Uuid::new_v4(),
            service_id,
            state: State::Running,
            last_update: time.checked_add_signed(Duration::seconds(2)).unwrap(),
            address: Some(SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 9876)),
            is_next: false,
            ..Default::default()
        };
        let deployment_queued = Deployment {
            id: Uuid::new_v4(),
            service_id,
            state: State::Queued,
            last_update: time.checked_add_signed(Duration::seconds(3)).unwrap(),
            address: None,
            is_next: false,
            ..Default::default()
        };
        let deployment_building = Deployment {
            id: Uuid::new_v4(),
            service_id,
            state: State::Building,
            last_update: time.checked_add_signed(Duration::seconds(4)).unwrap(),
            address: None,
            is_next: false,
            ..Default::default()
        };
        let deployment_built = Deployment {
            id: Uuid::new_v4(),
            service_id,
            state: State::Built,
            last_update: time.checked_add_signed(Duration::seconds(5)).unwrap(),
            address: None,
            is_next: true,
            ..Default::default()
        };
        let deployment_loading = Deployment {
            id: Uuid::new_v4(),
            service_id,
            state: State::Loading,
            last_update: time.checked_add_signed(Duration::seconds(6)).unwrap(),
            address: None,
            is_next: false,
            ..Default::default()
        };

        for deployment in [
            &deployment_crashed,
            &deployment_stopped,
            &deployment_running,
            &deployment_queued,
            &deployment_built,
            &deployment_building,
            &deployment_loading,
        ] {
            p.insert_deployment(deployment).await.unwrap();
        }

        p.cleanup_invalid_states().await.unwrap();

        let actual: Vec<_> = p
            .get_deployments(&service_id, 0, u32::MAX)
            .await
            .unwrap()
            .into_iter()
            .map(|deployment| (deployment.id, deployment.state))
            .collect();
        let expected = vec![
            (deployment_loading.id, State::Stopped),
            (deployment_built.id, State::Stopped),
            (deployment_building.id, State::Stopped),
            (deployment_queued.id, State::Stopped),
            (deployment_running.id, State::Running),
            (deployment_stopped.id, State::Stopped),
            (deployment_crashed.id, State::Crashed),
        ];

        assert_eq!(
            actual, expected,
            "invalid states should be moved to the stopped state"
        );
    }
    #[tokio::test(flavor = "multi_thread")]
    async fn fetching_runnable_deployments() {
        let (p, _) = Persistence::new_in_memory().await;
        let Mode::Local(pool) = &p.mode else {
            unreachable!()
        };
        let bar_id = add_service_named(pool, "bar").await.unwrap();
        let foo_id = add_service_named(pool, "foo").await.unwrap();
        let service_id = add_service(pool).await.unwrap();
        let service_id2 = add_service(pool).await.unwrap();

        let id_1 = Uuid::new_v4();
        let id_2 = Uuid::new_v4();
        let id_3 = Uuid::new_v4();
        let id_crashed = Uuid::new_v4();

        for deployment in &[
            Deployment {
                id: Uuid::new_v4(),
                service_id,
                state: State::Built,
                last_update: Utc.with_ymd_and_hms(2022, 4, 25, 4, 29, 33).unwrap(),
                address: None,
                is_next: false,
                ..Default::default()
            },
            Deployment {
                id: id_1,
                service_id: foo_id,
                state: State::Running,
                last_update: Utc.with_ymd_and_hms(2022, 4, 25, 4, 29, 44).unwrap(),
                address: None,
                is_next: false,
                ..Default::default()
            },
            Deployment {
                id: id_2,
                service_id: bar_id,
                state: State::Running,
                last_update: Utc.with_ymd_and_hms(2022, 4, 25, 4, 33, 48).unwrap(),
                address: None,
                is_next: true,
                ..Default::default()
            },
            Deployment {
                id: id_crashed,
                service_id: service_id2,
                state: State::Crashed,
                last_update: Utc.with_ymd_and_hms(2022, 4, 25, 4, 38, 52).unwrap(),
                address: None,
                is_next: true,
                ..Default::default()
            },
            Deployment {
                id: id_3,
                service_id: foo_id,
                state: State::Running,
                last_update: Utc.with_ymd_and_hms(2022, 4, 25, 4, 42, 32).unwrap(),
                address: None,
                is_next: false,
                ..Default::default()
            },
        ] {
            p.insert_deployment(deployment).await.unwrap();
        }

        let runnable = p.get_runnable_deployment(&id_1).await.unwrap();
        assert_eq!(
            runnable,
            Some(DeploymentRunnable {
                id: id_1,
                service_name: "foo".to_string(),
                service_id: foo_id,
                is_next: false,
            })
        );

        let runnable = p.get_runnable_deployment(&id_crashed).await.unwrap();
        assert_eq!(runnable, None);

        let runnable = p.get_all_runnable_deployments().await.unwrap();
        assert_eq!(
            runnable,
            [
                DeploymentRunnable {
                    id: id_3,
                    service_name: "foo".to_string(),
                    service_id: foo_id,
                    is_next: false,
                },
                DeploymentRunnable {
                    id: id_2,
                    service_name: "bar".to_string(),
                    service_id: bar_id,
                    is_next: true,
                },
                DeploymentRunnable {
                    id: id_1,
                    service_name: "foo".to_string(),
                    service_id: foo_id,
                    is_next: false,
                },
            ]
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn service() {
        let (p, _) = Persistence::new_in_memory().await;

        let service = p.get_or_create_service("dummy-service").await.unwrap();
        let service2 = p.get_or_create_service("dummy-service").await.unwrap();

        assert_eq!(service, service2, "service should only be added once");

        let get_result = p
            .get_service_by_name("dummy-service")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(service, get_result);

        p.delete_service(&service.id).await.unwrap();
        assert!(p
            .get_service_by_name("dummy-service")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn address_getter() {
        let (p, _) = Persistence::new_in_memory().await;
        let Mode::Local(pool) = &p.mode else {
            unreachable!()
        };
        let service_id = add_service_named(pool, "service-name").await.unwrap();
        let service_other_id = add_service_named(pool, "other-name").await.unwrap();

        sqlx::query(
            "INSERT INTO deployments (id, service_id, state, last_update, address) VALUES (?, ?, ?, ?, ?), (?, ?, ?, ?, ?), (?, ?, ?, ?, ?)",
        )
        // This running item should match
        .bind(Uuid::new_v4())
        .bind(service_id.to_string())
        .bind(State::Running)
        .bind(Utc::now())
        .bind("10.0.0.5:12356")
        // A stopped item should not match
        .bind(Uuid::new_v4())
        .bind(service_id.to_string())
        .bind(State::Stopped)
        .bind(Utc::now())
        .bind("10.0.0.5:9876")
        // Another service should not match
        .bind(Uuid::new_v4())
        .bind(service_other_id.to_string())
        .bind(State::Running)
        .bind(Utc::now())
        .bind("10.0.0.5:5678")
        .execute(pool)
        .await
        .unwrap();

        assert_eq!(
            SocketAddr::from(([10, 0, 0, 5], 12356)),
            p.get_address_for_service("service-name")
                .await
                .unwrap()
                .unwrap(),
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn active_deployment_getter() {
        let (p, _) = Persistence::new_in_memory().await;
        let Mode::Local(pool) = &p.mode else {
            unreachable!()
        };
        let service_id = add_service_named(pool, "service-name").await.unwrap();
        let id_1 = Uuid::new_v4();
        let id_2 = Uuid::new_v4();

        for deployment in &[
            Deployment {
                id: Uuid::new_v4(),
                service_id,
                state: State::Built,
                last_update: Utc.with_ymd_and_hms(2022, 4, 25, 4, 29, 33).unwrap(),
                address: None,
                is_next: false,
                ..Default::default()
            },
            Deployment {
                id: Uuid::new_v4(),
                service_id,
                state: State::Stopped,
                last_update: Utc.with_ymd_and_hms(2022, 4, 25, 4, 29, 44).unwrap(),
                address: None,
                is_next: false,
                ..Default::default()
            },
            Deployment {
                id: id_1,
                service_id,
                state: State::Running,
                last_update: Utc.with_ymd_and_hms(2022, 4, 25, 4, 33, 48).unwrap(),
                address: None,
                is_next: false,
                ..Default::default()
            },
            Deployment {
                id: Uuid::new_v4(),
                service_id,
                state: State::Crashed,
                last_update: Utc.with_ymd_and_hms(2022, 4, 25, 4, 38, 52).unwrap(),
                address: None,
                is_next: false,
                ..Default::default()
            },
            Deployment {
                id: id_2,
                service_id,
                state: State::Running,
                last_update: Utc.with_ymd_and_hms(2022, 4, 25, 4, 42, 32).unwrap(),
                address: None,
                is_next: true,
                ..Default::default()
            },
        ] {
            p.insert_deployment(deployment).await.unwrap();
        }

        let actual = p.get_active_deployments(&service_id).await.unwrap();

        assert_eq!(actual, vec![id_1, id_2]);
    }

    async fn add_service(pool: &SqlitePool) -> Result<Ulid> {
        add_service_named(pool, &get_random_name()).await
    }

    async fn add_service_named(pool: &SqlitePool, name: &str) -> Result<Ulid> {
        let service_id = Ulid::new();

        sqlx::query("INSERT INTO services (id, name) VALUES (?, ?)")
            .bind(service_id.to_string())
            .bind(name)
            .execute(pool)
            .await?;

        Ok(service_id)
    }

    fn get_random_name() -> String {
        rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(12)
            .map(char::from)
            .collect::<String>()
    }
}

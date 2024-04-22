use std::cmp::min;
use std::collections::{HashMap, VecDeque};
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::Future;
use http::StatusCode;
use opentelemetry::global;
use shuttle_backends::project_name::ProjectName;
use shuttle_common::models::error::ApiError;
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;
use tokio::time::{sleep, timeout};
use tracing::{error, field, info_span, trace, warn, Instrument, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use ulid::Ulid;

use crate::project::{self, *};
use crate::service::{self, GatewayContext, GatewayService};
use crate::worker::TaskRouter;
use crate::State;

// Default maximum _total_ time a task is allowed to run
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);
// Maximum time we'll wait for a task to successfully be sent down the channel
pub const TASK_SEND_TIMEOUT: Duration = Duration::from_secs(9);
// Maximum time before a task is considered degraded
pub const PROJECT_TASK_MAX_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

#[async_trait]
pub trait Task<Ctx>: Send {
    type Output;

    async fn poll(&mut self, ctx: Ctx) -> TaskResult<Self::Output>;
}

#[async_trait]
impl<Ctx, T> Task<Ctx> for Box<T>
where
    Ctx: Send + 'static,
    T: Task<Ctx> + ?Sized,
{
    type Output = T::Output;

    async fn poll(&mut self, ctx: Ctx) -> TaskResult<Self::Output> {
        self.as_mut().poll(ctx).await
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Project(#[from] project::Error),

    #[error(transparent)]
    Service(#[from] service::Error),

    #[error("{0}")]
    InvalidOperation(String),

    #[error("we are currently having issues processing your project request")]
    ServiceUnavailable,

    #[error(transparent)]
    SendError(#[from] SendError<BoxedTask>),
}

impl From<Error> for ApiError {
    fn from(error: Error) -> Self {
        let status_code = match error {
            Error::Project(e) => return e.into(),
            Error::Service(e) => return e.into(),
            Error::InvalidOperation(_) => StatusCode::BAD_REQUEST,
            Error::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            Error::SendError(err) => {
                error!(
                    error = &err as &dyn std::error::Error,
                    "failed to send request to the task router"
                );

                return Self::internal("we are unable to serve your request at this time");
            }
        };

        Self {
            message: error.to_string(),
            status_code: status_code.as_u16(),
        }
    }
}

#[must_use]
#[derive(Debug)]
pub enum TaskResult<R> {
    /// More work needs to be done
    Pending(R),
    /// No further work needed
    Done(R),
    /// Try again later
    TryAgain,
    /// Task has been cancelled
    Cancelled,
    /// Task has failed
    Err(Error),
}

impl<R> TaskResult<R> {
    pub fn ok(&self) -> Option<&R> {
        match self {
            Self::Pending(r) | Self::Done(r) => Some(r),
            _ => None,
        }
    }

    pub fn is_done(&self) -> bool {
        match self {
            Self::Done(_) | Self::Cancelled | Self::Err(_) => true,
            Self::TryAgain | Self::Pending(_) => false,
        }
    }
}

pub fn run<F, Fut>(f: F) -> impl Task<ProjectContext, Output = Project>
where
    F: FnMut(ProjectContext) -> Fut + Send + 'static,
    Fut: Future<Output = TaskResult<Project>> + Send + 'static,
{
    RunFn {
        f,
        _output: PhantomData,
    }
}

pub fn destroy() -> impl Task<ProjectContext, Output = Project> {
    run(|ctx| async move {
        match ctx.state.destroy() {
            Ok(state) => TaskResult::Done(state),
            Err(err) => TaskResult::Err(err.into()),
        }
    })
}

pub fn start() -> impl Task<ProjectContext, Output = Project> {
    run(|ctx| async move {
        match ctx.state.start() {
            Ok(state) => TaskResult::Done(state),
            Err(err) => TaskResult::Err(err.into()),
        }
    })
}

/// Will force restart a project no matter the state it is in
pub fn restart(project_id: Ulid) -> impl Task<ProjectContext, Output = Project> {
    run(move |ctx| async move {
        let state = ctx
            .state
            .container()
            .and_then(|container| ProjectCreating::from_container(container, 0).ok())
            .unwrap_or_else(|| {
                ProjectCreating::new_with_random_initial_key(ctx.project_name, project_id, 1)
            });

        TaskResult::Done(Project::Creating(state))
    })
}

pub fn start_idle_deploys() -> impl Task<ProjectContext, Output = Project> {
    run(|ctx| async move {
        match ctx.state {
            Project::Ready(mut ready) => {
                ready
                    .start_last_deploy(ctx.gateway.get_jwt().await, ctx.admin_secret.clone())
                    .await;
                TaskResult::Done(Project::Ready(ready))
            }
            other => TaskResult::Done(other),
        }
    })
}

pub fn run_until_done() -> impl Task<ProjectContext, Output = Project> {
    RunUntilDone::default()
}

pub fn delete_project() -> impl Task<ProjectContext, Output = Project> {
    DeleteProject
}

pub struct TaskBuilder {
    project_name: Option<ProjectName>,
    operation_name: Option<String>,
    service: Arc<GatewayService>,
    tasks: VecDeque<BoxedTask<ProjectContext, Project>>,
}

impl TaskBuilder {
    pub fn new(service: Arc<GatewayService>, operation_name: Option<String>) -> Self {
        Self {
            operation_name,
            service,
            project_name: None,
            tasks: VecDeque::new(),
        }
    }

    pub fn project(mut self, name: ProjectName) -> Self {
        self.project_name = Some(name);
        self
    }

    pub fn and_then<T>(mut self, task: T) -> Self
    where
        T: Task<ProjectContext, Output = Project> + 'static,
    {
        self.tasks.push_back(Box::new(task));
        self
    }

    pub fn build(mut self) -> BoxedTask {
        self.tasks.push_back(Box::<RunUntilDone>::default());

        let cx = Span::current().context();
        let mut tracing_context: HashMap<String, String> = Default::default();

        opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&cx, &mut tracing_context);
        });

        Box::new(WithTimeout::on(
            DEFAULT_TIMEOUT,
            ProjectTask {
                project_name: self.project_name.expect("project_name is required"),
                service: self.service,
                tasks: self.tasks,
                tracing_context,
                operation_name: self.operation_name,
            },
        ))
    }

    pub async fn send(self, sender: &Sender<BoxedTask>) -> Result<TaskHandle, Error> {
        let project_name = self.project_name.clone().expect("project_name is required");
        let task_router = self.service.task_router();
        let (task, handle) = AndThenNotify::after(self.build());
        let task = Route::to(project_name, Box::new(task), task_router);
        match timeout(TASK_SEND_TIMEOUT, sender.send(Box::new(task))).await {
            Ok(Ok(_)) => Ok(handle),
            _ => Err(Error::ServiceUnavailable),
        }
    }
}

pub struct Route {
    project_name: ProjectName,
    inner: Option<BoxedTask>,
    router: TaskRouter,
}

impl Route {
    pub fn to(project_name: ProjectName, what: BoxedTask, router: TaskRouter) -> Self {
        Self {
            project_name,
            inner: Some(what),
            router,
        }
    }
}

#[async_trait]
impl Task<()> for Route {
    type Output = ();

    async fn poll(&mut self, _ctx: ()) -> TaskResult<Self::Output> {
        if let Some(task) = self.inner.take() {
            match self.router.route(&self.project_name, task).await {
                Ok(_) => TaskResult::Done(()),
                Err(err) => TaskResult::Err(err.into()),
            }
        } else {
            TaskResult::Done(())
        }
    }
}

pub struct RunFn<F, O> {
    f: F,
    _output: PhantomData<O>,
}

#[async_trait]
impl<F, Fut> Task<ProjectContext> for RunFn<F, Fut>
where
    F: FnMut(ProjectContext) -> Fut + Send,
    Fut: Future<Output = TaskResult<Project>> + Send,
{
    type Output = Project;

    async fn poll(&mut self, ctx: ProjectContext) -> TaskResult<Self::Output> {
        (self.f)(ctx).await
    }
}

/// Advance a project's state until it's returning `is_done`
#[derive(Default)]
pub struct RunUntilDone {
    tries: u32,
}

#[async_trait]
impl Task<ProjectContext> for RunUntilDone {
    type Output = Project;

    async fn poll(&mut self, ctx: ProjectContext) -> TaskResult<Self::Output> {
        // Don't overload Docker with requests. Therefore backoff with each try up to 30 seconds
        if self.tries > 0 {
            let backoff = min(2_u64.pow(self.tries), 300);

            sleep(Duration::from_millis(backoff)).await;
        }
        self.tries += 1;

        // Make sure the project state has not changed from Docker
        // Else we will make assumptions when trying to run next which can cause a failure
        let project = match refresh_with_retry(ctx.state, &ctx.gateway).await {
            Ok(project) => project,
            Err(error) => return TaskResult::Err(error.into()),
        };

        match project {
            Project::Errored(_)
            | Project::Destroyed(_)
            | Project::Stopped(_)
            | Project::Deleted => TaskResult::Done(project),
            Project::Ready(_) => match project.next(&ctx.gateway).await.unwrap() {
                Project::Ready(ready) => TaskResult::Done(Project::Ready(ready)),
                other => TaskResult::Pending(other),
            },
            Project::Restarting(restarting) if restarting.exhausted() => {
                trace!("skipping project that restarted too many times");
                TaskResult::Done(Project::Restarting(restarting))
            }
            _ => TaskResult::Pending(project.next(&ctx.gateway).await.unwrap()),
        }
    }
}

pub struct DeleteProject;

#[async_trait]
impl Task<ProjectContext> for DeleteProject {
    type Output = Project;

    async fn poll(&mut self, ctx: ProjectContext) -> TaskResult<Self::Output> {
        // Make sure the project state has not changed from Docker
        // Else we will make assumptions when trying to run next which can cause a failure
        let project = match refresh_with_retry(ctx.state, &ctx.gateway).await {
            Ok(project) => project,
            Err(error) => return TaskResult::Err(error.into()),
        };

        match project {
            Project::Errored(_)
            | Project::Destroyed(_)
            | Project::Stopped(_)
            | Project::Ready(_) => match project.delete(&ctx.gateway).await {
                Ok(()) => TaskResult::Done(Project::Deleted),
                Err(error) => TaskResult::Err(error.into()),
            },
            _ => TaskResult::Err(Error::InvalidOperation(
                "project is not in a valid state to be deleted".to_string(),
            )),
        }
    }
}

pub struct TaskHandle {
    rx: oneshot::Receiver<()>,
}

impl Future for TaskHandle {
    type Output = ();

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        Pin::new(&mut self.rx).poll(cx).map(|_| ())
    }
}

pub struct AndThenNotify<T> {
    inner: T,
    notify: Option<oneshot::Sender<()>>,
}

impl<T> AndThenNotify<T> {
    pub fn after(task: T) -> (Self, TaskHandle) {
        let (tx, rx) = oneshot::channel();
        (
            Self {
                inner: task,
                notify: Some(tx),
            },
            TaskHandle { rx },
        )
    }
}

#[async_trait]
impl<T, Ctx> Task<Ctx> for AndThenNotify<T>
where
    Ctx: Send + 'static,
    T: Task<Ctx>,
{
    type Output = T::Output;

    async fn poll(&mut self, ctx: Ctx) -> TaskResult<Self::Output> {
        let out = self.inner.poll(ctx).await;

        if out.is_done() {
            let _ = self.notify.take().unwrap().send(());
        }

        out
    }
}

pub struct WithTimeout<T> {
    inner: T,
    start: Option<Instant>,
    timeout: Duration,
}

impl<T> WithTimeout<T> {
    pub fn on(timeout: Duration, inner: T) -> Self {
        Self {
            inner,
            start: None,
            timeout,
        }
    }
}

#[async_trait]
impl<T, Ctx> Task<Ctx> for WithTimeout<T>
where
    Ctx: Send + 'static,
    T: Task<Ctx>,
{
    type Output = T::Output;

    async fn poll(&mut self, ctx: Ctx) -> TaskResult<Self::Output> {
        if self.start.is_none() {
            self.start = Some(Instant::now());
        }

        if Instant::now() - *self.start.as_ref().unwrap() > self.timeout {
            warn!(
                "task has timed out: was running for more than {}s",
                self.timeout.as_secs()
            );
            return TaskResult::Cancelled;
        }

        self.inner.poll(ctx).await
    }
}

/// A collection of tasks scoped to a specific project.
///
/// All the tasks in the collection are run to completion. If an error
/// is encountered, the `ProjectTask` completes early passing through
/// the error. The value returned by the inner tasks upon their
/// completion is committed back to persistence through
/// [GatewayService].
pub struct ProjectTask {
    project_name: ProjectName,
    service: Arc<GatewayService>,
    tasks: VecDeque<BoxedTask<ProjectContext, Project>>,
    tracing_context: HashMap<String, String>,
    operation_name: Option<String>,
}

/// A context for tasks which are scoped to a specific project.
///
/// This will be always instantiated with the latest known state of
/// the project and gives access to the broader gateway context.
#[derive(Clone)]
pub struct ProjectContext {
    /// The name of the project this task is about
    pub project_name: ProjectName,
    /// The gateway context in which this task is running
    pub gateway: GatewayContext,
    /// The last known state of the project
    pub state: Project,
    /// The secret needed to communicate with the project
    pub admin_secret: String,
}

pub type BoxedTask<Ctx = (), O = ()> = Box<dyn Task<Ctx, Output = O>>;

#[async_trait]
impl Task<()> for ProjectTask {
    type Output = ();

    async fn poll(&mut self, _: ()) -> TaskResult<Self::Output> {
        if self.tasks.is_empty() {
            return TaskResult::Done(());
        }

        let ctx = self.service.context().clone();

        let project = match self.service.find_project_by_name(&self.project_name).await {
            Ok(project) => project,
            Err(err) => return TaskResult::Err(err.into()),
        };

        let admin_secret = match self
            .service
            .control_key_from_project_name(&self.project_name)
            .await
        {
            Ok(admin_secret) => admin_secret,
            Err(err) => return TaskResult::Err(err.into()),
        };

        let project_ctx = ProjectContext {
            project_name: self.project_name.clone(),
            gateway: ctx,
            state: project.state,
            admin_secret,
        };

        let parent_cx =
            global::get_text_map_propagator(|propagator| propagator.extract(&self.tracing_context));

        let span = info_span!(
            "polling project",
            shuttle.project.name = %project_ctx.project_name,
            shuttle.operation_name = field::Empty,
            ctx.state = project_ctx.state.state(),
            ctx.state_after = field::Empty,
            ctx.operation_name = field::Empty
        );
        span.set_parent(parent_cx);

        async {
            let task = self.tasks.front_mut().unwrap();
            let timeout = sleep(PROJECT_TASK_MAX_IDLE_TIMEOUT);
            let res = {
                let mut poll = task.poll(project_ctx);
                tokio::select! {
                    res = &mut poll => res,
                    _ = timeout => {
                        warn!(
                            shuttle.project.name = %self.project_name,
                            "a task has been idling for a long time"
                        );
                        poll.await
                    }
                }
            };

            if let Some(update) = res.ok() {
                let span = Span::current();
                span.record("ctx.state_after", update.state());

                match self
                    .service
                    .update_project(&self.project_name, update)
                    .await
                {
                    Ok(_) => {}
                    Err(err) => {
                        error!(
                            error = &err as &dyn std::error::Error,
                            "could not update project state"
                        );
                        return TaskResult::Err(err.into());
                    }
                }
            }

            match res {
                TaskResult::Pending(_) => TaskResult::Pending(()),
                TaskResult::TryAgain => TaskResult::TryAgain,
                TaskResult::Done(_) => {
                    let _ = self.tasks.pop_front().unwrap();
                    if self.tasks.is_empty() {
                        // Should coincide with the end of a project task started by
                        // an API call or the ambulance.
                        if let Some(operation) = &self.operation_name {
                            Span::current().record("ctx.operation_name", operation);
                            Span::current().record("shuttle.operation_name", operation);
                        }
                        TaskResult::Done(())
                    } else {
                        TaskResult::Pending(())
                    }
                }
                TaskResult::Cancelled => {
                    if let Some(operation) = &self.operation_name {
                        Span::current().record("ctx.operation_name", operation);
                        Span::current().record("shuttle.operation_name", operation);
                    }
                    TaskResult::Cancelled
                }
                TaskResult::Err(err) => {
                    if let Some(operation) = &self.operation_name {
                        Span::current().record("ctx.operation_name", operation);
                        Span::current().record("shuttle.operation_name", operation);
                    }
                    error!(
                        error = &err as &dyn std::error::Error,
                        "project task failure"
                    );
                    TaskResult::Err(err)
                }
            }
        }
        .instrument(span)
        .await
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    struct NeverEnding;

    #[async_trait]
    impl Task<()> for NeverEnding {
        type Output = ();

        async fn poll(&mut self, _ctx: ()) -> TaskResult<Self::Output> {
            TaskResult::Pending(())
        }
    }

    #[tokio::test]
    async fn task_with_timeout() -> anyhow::Result<()> {
        let timeout = Duration::from_secs(1);

        let mut task_with_timeout = WithTimeout::on(timeout, NeverEnding);

        let start = Instant::now();

        while let TaskResult::Pending(()) = task_with_timeout.poll(()).await {
            assert!(Instant::now() - start <= timeout + Duration::from_secs(1));
        }

        assert!(matches!(
            task_with_timeout.poll(()).await,
            TaskResult::Cancelled
        ));

        Ok(())
    }
}

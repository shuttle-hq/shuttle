use futures::Future;
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;
use tokio::time::{sleep, timeout};
use tracing::{error, trace, trace_span, warn};
use uuid::Uuid;

use crate::project::*;
use crate::service::{GatewayContext, GatewayService};
use crate::worker::TaskRouter;
use crate::{AccountName, Error, ErrorKind, ProjectName, Refresh, State};

// Default maximum _total_ time a task is allowed to run
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);
// Maximum time we'll wait for a task to successfully be sent down the channel
pub const TASK_SEND_TIMEOUT: Duration = Duration::from_secs(9);
// Maximum time before a task is considered degraded
pub const PROJECT_TASK_MAX_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

#[async_trait]
pub trait Task<Ctx>: Send {
    type Output;

    type Error;

    async fn poll(&mut self, ctx: Ctx) -> TaskResult<Self::Output, Self::Error>;
}

#[async_trait]
impl<Ctx, T> Task<Ctx> for Box<T>
where
    Ctx: Send + 'static,
    T: Task<Ctx> + ?Sized,
{
    type Output = T::Output;

    type Error = T::Error;

    async fn poll(&mut self, ctx: Ctx) -> TaskResult<Self::Output, Self::Error> {
        self.as_mut().poll(ctx).await
    }
}

#[must_use]
#[derive(Debug, PartialEq, Eq)]
pub enum TaskResult<R, E> {
    /// More work needs to be done
    Pending(R),
    /// No further work needed
    Done(R),
    /// Try again later
    TryAgain,
    /// Task has been cancelled
    Cancelled,
    /// Task has failed
    Err(E),
}

impl<R, E> TaskResult<R, E> {
    pub fn ok(self) -> Option<R> {
        match self {
            Self::Pending(r) | Self::Done(r) => Some(r),
            _ => None,
        }
    }

    pub fn to_str(&self) -> &str {
        match self {
            Self::Pending(_) => "pending",
            Self::Done(_) => "done",
            Self::TryAgain => "try again",
            Self::Cancelled => "cancelled",
            Self::Err(_) => "error",
        }
    }

    pub fn is_done(&self) -> bool {
        match self {
            Self::Done(_) | Self::Cancelled | Self::Err(_) => true,
            Self::TryAgain | Self::Pending(_) => false,
        }
    }

    pub fn as_ref(&self) -> TaskResult<&R, &E> {
        match self {
            Self::Pending(r) => TaskResult::Pending(r),
            Self::Done(r) => TaskResult::Done(r),
            Self::TryAgain => TaskResult::TryAgain,
            Self::Cancelled => TaskResult::Cancelled,
            Self::Err(e) => TaskResult::Err(e),
        }
    }
}

pub fn run<F, Fut>(f: F) -> impl Task<ProjectContext, Output = Project, Error = Error>
where
    F: FnMut(ProjectContext) -> Fut + Send + 'static,
    Fut: Future<Output = TaskResult<Project, Error>> + Send + 'static,
{
    RunFn {
        f,
        _output: PhantomData,
    }
}

pub fn destroy() -> impl Task<ProjectContext, Output = Project, Error = Error> {
    run(|ctx| async move {
        match ctx.state.destroy() {
            Ok(state) => TaskResult::Done(state),
            Err(err) => TaskResult::Err(err),
        }
    })
}

pub fn start() -> impl Task<ProjectContext, Output = Project, Error = Error> {
    run(|ctx| async move {
        match ctx.state.start() {
            Ok(state) => TaskResult::Done(state),
            Err(err) => TaskResult::Err(err),
        }
    })
}

pub fn start_idle_deploys() -> impl Task<ProjectContext, Output = Project, Error = Error> {
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

pub fn run_until_done() -> impl Task<ProjectContext, Output = Project, Error = Error> {
    RunUntilDone
}

pub fn delete_project() -> impl Task<ProjectContext, Output = Project, Error = Error> {
    DeleteProject
}

pub struct TaskBuilder {
    project_name: Option<ProjectName>,
    service: Arc<GatewayService>,
    timeout: Option<Duration>,
    tasks: VecDeque<BoxedTask<ProjectContext, Project>>,
}

impl TaskBuilder {
    pub fn new(service: Arc<GatewayService>) -> Self {
        Self {
            service,
            project_name: None,
            timeout: None,
            tasks: VecDeque::new(),
        }
    }

    pub fn project(mut self, name: ProjectName) -> Self {
        self.project_name = Some(name);
        self
    }

    pub fn and_then<T>(mut self, task: T) -> Self
    where
        T: Task<ProjectContext, Output = Project, Error = Error> + 'static,
    {
        self.tasks.push_back(Box::new(task));
        self
    }

    pub fn with_timeout(mut self, duration: Duration) -> Self {
        self.timeout = Some(duration);
        self
    }

    pub fn build(mut self) -> BoxedTask {
        self.tasks.push_back(Box::new(RunUntilDone));

        let timeout = self.timeout.unwrap_or(DEFAULT_TIMEOUT);

        Box::new(WithTimeout::on(
            timeout,
            ProjectTask {
                uuid: Uuid::new_v4(),
                project_name: self.project_name.expect("project_name is required"),
                service: self.service,
                tasks: self.tasks,
            },
        ))
    }

    pub async fn send(self, sender: &Sender<BoxedTask>) -> Result<TaskHandle, Error> {
        let project_name = self.project_name.clone().expect("project_name is required");
        let task_router = self.service.task_router();
        let (task, handle) = AndThenNotify::after(self.build());
        let task = Route::<BoxedTask>::to(project_name, Box::new(task), task_router);
        match timeout(TASK_SEND_TIMEOUT, sender.send(Box::new(task))).await {
            Ok(Ok(_)) => Ok(handle),
            _ => Err(Error::from_kind(ErrorKind::ServiceUnavailable)),
        }
    }
}

pub struct Route<T> {
    project_name: ProjectName,
    inner: Option<T>,
    router: TaskRouter<T>,
}

impl<T> Route<T> {
    pub fn to(project_name: ProjectName, what: T, router: TaskRouter<T>) -> Self {
        Self {
            project_name,
            inner: Some(what),
            router,
        }
    }
}

#[async_trait]
impl Task<()> for Route<BoxedTask> {
    type Output = ();

    type Error = Error;

    async fn poll(&mut self, _ctx: ()) -> TaskResult<Self::Output, Self::Error> {
        if let Some(task) = self.inner.take() {
            match self.router.route(&self.project_name, task).await {
                Ok(_) => TaskResult::Done(()),
                Err(_) => TaskResult::Err(Error::from_kind(ErrorKind::Internal)),
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
    Fut: Future<Output = TaskResult<Project, Error>> + Send,
{
    type Output = Project;

    type Error = Error;

    async fn poll(&mut self, ctx: ProjectContext) -> TaskResult<Self::Output, Self::Error> {
        (self.f)(ctx).await
    }
}

/// Advance a project's state until it's returning `is_done`
pub struct RunUntilDone;

#[async_trait]
impl Task<ProjectContext> for RunUntilDone {
    type Output = Project;

    type Error = Error;

    async fn poll(&mut self, ctx: ProjectContext) -> TaskResult<Self::Output, Self::Error> {
        // Make sure the project state has not changed from Docker
        // Else we will make assumptions when trying to run next which can cause a failure
        let project = match ctx.state.refresh(&ctx.gateway).await {
            Ok(project) => project,
            Err(error) => return TaskResult::Err(error),
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

    type Error = Error;

    async fn poll(&mut self, ctx: ProjectContext) -> TaskResult<Self::Output, Self::Error> {
        // Make sure the project state has not changed from Docker
        // Else we will make assumptions when trying to run next which can cause a failure
        let project = match ctx.state.refresh(&ctx.gateway).await {
            Ok(project) => project,
            Err(error) => return TaskResult::Err(error),
        };

        match project {
            Project::Errored(_)
            | Project::Destroyed(_)
            | Project::Stopped(_)
            | Project::Ready(_) => match project.delete(&ctx.gateway).await {
                Ok(()) => TaskResult::Done(Project::Deleted),
                Err(error) => TaskResult::Err(Error::source(ErrorKind::DeleteProjectFailed, error)),
            },
            _ => TaskResult::Err(Error::custom(
                ErrorKind::InvalidOperation,
                "project is not in a valid state to be deleted",
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

    type Error = T::Error;

    async fn poll(&mut self, ctx: Ctx) -> TaskResult<Self::Output, Self::Error> {
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

    type Error = T::Error;

    async fn poll(&mut self, ctx: Ctx) -> TaskResult<Self::Output, Self::Error> {
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
pub struct ProjectTask<T> {
    uuid: Uuid,
    project_name: ProjectName,
    service: Arc<GatewayService>,
    tasks: VecDeque<T>,
}

impl<T> ProjectTask<T> {
    pub fn uuid(&self) -> &Uuid {
        &self.uuid
    }
}

/// A context for tasks which are scoped to a specific project.
///
/// This will be always instantiated with the latest known state of
/// the project and gives access to the broader gateway context.
#[derive(Clone)]
pub struct ProjectContext {
    /// The name of the project this task is about
    pub project_name: ProjectName,
    /// The name of the user the project belongs to
    pub account_name: AccountName,
    /// The gateway context in which this task is running
    pub gateway: GatewayContext,
    /// The last known state of the project
    pub state: Project,
    /// The secret needed to communicate with the project
    pub admin_secret: String,
}

pub type BoxedTask<Ctx = (), O = ()> = Box<dyn Task<Ctx, Output = O, Error = Error>>;

#[async_trait]
impl<T> Task<()> for ProjectTask<T>
where
    T: Task<ProjectContext, Output = Project, Error = Error>,
{
    type Output = ();

    type Error = Error;

    async fn poll(&mut self, _: ()) -> TaskResult<Self::Output, Self::Error> {
        if self.tasks.is_empty() {
            return TaskResult::Done(());
        }

        let ctx = self.service.context();

        let project = match self.service.find_project(&self.project_name).await {
            Ok(project) => project,
            Err(err) => return TaskResult::Err(err),
        };

        let account_name = match self
            .service
            .account_name_from_project(&self.project_name)
            .await
        {
            Ok(account_name) => account_name,
            Err(err) => return TaskResult::Err(err),
        };
        let admin_secret = match self
            .service
            .control_key_from_project_name(&self.project_name)
            .await
        {
            Ok(account_name) => account_name,
            Err(err) => return TaskResult::Err(err),
        };

        let project_ctx = ProjectContext {
            project_name: self.project_name.clone(),
            account_name: account_name.clone(),
            gateway: ctx,
            state: project.state,
            admin_secret,
        };

        let span = trace_span!(
            "polling project",
            ctx.project = ?project_ctx.project_name,
            ctx.account = ?project_ctx.account_name,
            ctx.state = project_ctx.state.state()
        );
        let _ = span.enter();

        let task = self.tasks.front_mut().unwrap();

        let timeout = sleep(PROJECT_TASK_MAX_IDLE_TIMEOUT);
        let res = {
            let mut poll = task.poll(project_ctx);
            tokio::select! {
                res = &mut poll => res,
                _ = timeout => {
                    warn!(
                        project_name = ?self.project_name,
                        account_name = ?account_name,
                        "a task has been idling for a long time"
                    );
                    poll.await
                }
            }
        };

        if let Some(update) = res.as_ref().ok() {
            trace!(new_state = ?update.state(), "new state");
            match self
                .service
                .update_project(&self.project_name, update)
                .await
            {
                Ok(_) => {
                    trace!(new_state = ?update.state(), "successfully updated project state");
                }
                Err(err) => {
                    error!(err = %err, "could not update project state");
                    return TaskResult::Err(err);
                }
            }
        }

        trace!(result = res.to_str(), "poll result");

        match res {
            TaskResult::Pending(_) => TaskResult::Pending(()),
            TaskResult::TryAgain => TaskResult::TryAgain,
            TaskResult::Done(_) => {
                let _ = self.tasks.pop_front().unwrap();
                if self.tasks.is_empty() {
                    TaskResult::Done(())
                } else {
                    TaskResult::Pending(())
                }
            }
            TaskResult::Cancelled => TaskResult::Cancelled,
            TaskResult::Err(err) => {
                error!(err = %err, "project task failure");
                TaskResult::Err(err)
            }
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    struct NeverEnding;

    #[async_trait]
    impl Task<()> for NeverEnding {
        type Output = ();

        type Error = ();

        async fn poll(&mut self, _ctx: ()) -> TaskResult<Self::Output, Self::Error> {
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

        assert_eq!(task_with_timeout.poll(()).await, TaskResult::Cancelled);

        Ok(())
    }
}

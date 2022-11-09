use futures::Future;
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::Sender;
use tokio::time::{sleep, timeout};
use tracing::warn;
use uuid::Uuid;

use crate::project::*;
use crate::service::{GatewayContext, GatewayService};
use crate::{AccountName, EndState, Error, ErrorKind, ProjectName, Refresh, State};

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

pub fn refresh() -> impl Task<ProjectContext, Output = Project, Error = Error> {
    run(|ctx: ProjectContext| async move {
        match ctx.state.refresh(&ctx.gateway).await {
            Ok(new) => TaskResult::Done(new),
            Err(err) => TaskResult::Err(err),
        }
    })
}

pub fn destroy() -> impl Task<ProjectContext, Output = Project, Error = Error> {
    run(|ctx| async move {
        match ctx.state.destroy() {
            Ok(state) => TaskResult::Done(state),
            Err(err) => TaskResult::Err(err),
        }
    })
}

pub fn check_health() -> impl Task<ProjectContext, Output = Project, Error = Error> {
    run(|ctx| async move {
        if let Project::Ready(mut ready) = ctx.state {
            if ready.is_healthy().await {
                TaskResult::Done(Project::Ready(ready))
            } else {
                match Project::Ready(ready).refresh(&ctx.gateway).await {
                    Ok(update) => TaskResult::Done(update),
                    Err(err) => TaskResult::Err(err),
                }
            }
        } else {
            TaskResult::Err(Error::from_kind(ErrorKind::NotReady))
        }
    })
}

pub fn run_until_done() -> impl Task<ProjectContext, Output = Project, Error = Error> {
    RunUntilDone
}

pub struct TaskBuilder {
    project_name: Option<ProjectName>,
    account_name: Option<AccountName>,
    service: Arc<GatewayService>,
    timeout: Option<Duration>,
    tasks: VecDeque<BoxedTask<ProjectContext, Project>>,
}

impl TaskBuilder {
    pub fn new(service: Arc<GatewayService>) -> Self {
        Self {
            service,
            project_name: None,
            account_name: None,
            timeout: None,
            tasks: VecDeque::new(),
        }
    }
}

impl TaskBuilder {
    pub fn project(mut self, name: ProjectName) -> Self {
        self.project_name = Some(name);
        self
    }

    pub fn account(mut self, name: AccountName) -> Self {
        self.account_name = Some(name);
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
                account_name: self.account_name.expect("account_name is required"),
                service: self.service,
                tasks: self.tasks,
            },
        ))
    }

    pub async fn send(self, sender: &Sender<BoxedTask>) -> Result<(), Error> {
        match timeout(TASK_SEND_TIMEOUT, sender.send(self.build())).await {
            Ok(Ok(_)) => Ok(()),
            _ => Err(Error::from_kind(ErrorKind::ServiceUnavailable)),
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
        if !<Project as EndState<GatewayContext>>::is_done(&ctx.state) {
            TaskResult::Pending(ctx.state.next(&ctx.gateway).await.unwrap())
        } else {
            TaskResult::Done(ctx.state)
        }
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
    account_name: AccountName,
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

        let project_ctx = ProjectContext {
            project_name: self.project_name.clone(),
            account_name: self.account_name.clone(),
            gateway: ctx,
            state: project,
        };

        let task = self.tasks.front_mut().unwrap();

        let timeout = sleep(PROJECT_TASK_MAX_IDLE_TIMEOUT);
        let res = {
            let mut poll = task.poll(project_ctx);
            tokio::select! {
                res = &mut poll => res,
                _ = timeout => {
                    warn!(
                        project_name = ?self.project_name,
                        account_name = ?self.account_name,
                        "a task has been idling for a long time"
                    );
                    poll.await
                }
            }
        };

        if let Some(update) = res.as_ref().ok() {
            match self
                .service
                .update_project(&self.project_name, update)
                .await
            {
                Ok(_) => {}
                Err(err) => return TaskResult::Err(err),
            }
        }

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
            TaskResult::Err(err) => TaskResult::Err(err),
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

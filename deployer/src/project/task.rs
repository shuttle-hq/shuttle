use async_trait::async_trait;
use futures::Future;
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::pin::Pin;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;
use tokio::time::{sleep, timeout};
use tracing::{error, info_span, trace, warn};
use ulid::Ulid;

use crate::deployment::persistence::dal::Dal;

use super::docker::{DockerContext, ServiceDockerContext};
use super::error::Error;
use super::service::state::machine::{EndState, Refresh, State};
use super::service::ServiceState;
use super::worker::TaskRouter;

// Default maximum _total_ time a task is allowed to run
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);
// Maximum time we'll wait for a task to successfully be sent down the channel
pub const TASK_SEND_TIMEOUT: Duration = Duration::from_secs(9);
// Maximum time before a task is considered degraded
pub const SERVICE_TASK_MAX_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

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

pub fn run<F, Fut>(f: F) -> impl Task<ServiceTaskContext, Output = ServiceState, Error = Error>
where
    F: FnMut(ServiceTaskContext) -> Fut + Send + 'static,
    Fut: Future<Output = TaskResult<ServiceState, Error>> + Send + 'static,
{
    RunFn {
        f,
        _output: PhantomData,
    }
}

pub fn refresh() -> impl Task<ServiceTaskContext, Output = ServiceState, Error = Error> {
    run(|ctx: ServiceTaskContext| async move {
        match ctx.state.refresh(&ctx.docker_context).await {
            Ok(new) => TaskResult::Done(new),
            Err(err) => TaskResult::Err(Error::Service(err)),
        }
    })
}

pub fn destroy() -> impl Task<ServiceTaskContext, Output = ServiceState, Error = Error> {
    run(|ctx| async move {
        match ctx.state.destroy() {
            Ok(state) => TaskResult::Done(state),
            Err(err) => TaskResult::Err(Error::Service(err)),
        }
    })
}

pub fn start() -> impl Task<ServiceTaskContext, Output = ServiceState, Error = Error> {
    run(|ctx| async move {
        match ctx.state.start() {
            Ok(state) => TaskResult::Done(state),
            Err(err) => TaskResult::Err(Error::Service(err)),
        }
    })
}

pub fn check_health() -> impl Task<ServiceTaskContext, Output = ServiceState, Error = Error> {
    run(|ctx| async move {
        match ctx.state.refresh(&ctx.docker_context).await {
            Ok(ServiceState::Ready(mut ready)) => {
                if ready
                    .is_healthy(ctx.docker_context.runtime_manager())
                    .await
                    .is_ok()
                {
                    TaskResult::Done(ServiceState::Ready(ready))
                } else {
                    TaskResult::Done(ServiceState::Ready(ready).reboot().unwrap())
                }
            }
            Ok(update) => TaskResult::Done(update),
            Err(err) => TaskResult::Err(Error::Service(err)),
        }
    })
}

pub fn run_until_done() -> impl Task<ServiceTaskContext, Output = ServiceState, Error = Error> {
    RunUntilDone
}

pub struct TaskBuilder<D: Dal + Send + Sync + 'static> {
    dal: D,
    service_id: Option<Ulid>,
    service_context: Option<ServiceDockerContext>,
    timeout: Option<Duration>,
    task_router: Option<TaskRouter<BoxedTask>>,
    tasks: VecDeque<BoxedTask<ServiceTaskContext, ServiceState>>,
}

impl<D: Dal + Send + Sync + 'static> TaskBuilder<D> {
    pub fn new(dal: D) -> Self {
        Self {
            dal,
            service_id: None,
            service_context: None,
            timeout: None,
            task_router: None,
            tasks: VecDeque::new(),
        }
    }

    pub fn service_id(mut self, service_id: Ulid) -> Self {
        self.service_id = Some(service_id);
        self
    }

    pub fn service_context(mut self, service_context: ServiceDockerContext) -> Self {
        self.service_context = Some(service_context);
        self
    }

    pub fn task_router(mut self, task_router: TaskRouter<BoxedTask>) -> Self {
        self.task_router = Some(task_router);
        self
    }

    pub fn and_then<T>(mut self, task: T) -> Self
    where
        T: Task<ServiceTaskContext, Output = ServiceState, Error = Error> + 'static,
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
            ServiceTask {
                service_id: self.service_id.expect("to provide a service id"),
                docker_context: self.service_context.expect("to provide a service context"),
                dal: self.dal,
                tasks: self.tasks,
            },
        ))
    }

    pub async fn send(self, sender: &Sender<BoxedTask>) -> Result<TaskHandle, Error> {
        let service_id = self.service_id.clone().expect("service id is required");
        let task_router = self.task_router.clone().expect("task router is required");
        let (task, handle) = AndThenNotify::after(self.build());
        let task = Route::<BoxedTask>::to(service_id, Box::new(task), task_router);
        match timeout(TASK_SEND_TIMEOUT, sender.send(Box::new(task))).await {
            Ok(Ok(_)) => Ok(handle),
            _ => Err(Error::ServiceUnavailable),
        }
    }
}

pub struct Route<T> {
    service_id: Ulid,
    inner: Option<T>,
    router: TaskRouter<T>,
}

impl<T> Route<T> {
    pub fn to(service_id: Ulid, what: T, router: TaskRouter<T>) -> Self {
        Self {
            service_id,
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
            match self.router.route(&self.service_id, task).await {
                Ok(_) => TaskResult::Done(()),
                Err(_) => TaskResult::Err(Error::TaskInternal),
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
impl<F, Fut> Task<ServiceTaskContext> for RunFn<F, Fut>
where
    F: FnMut(ServiceTaskContext) -> Fut + Send,
    Fut: Future<Output = TaskResult<ServiceState, Error>> + Send,
{
    type Output = ServiceState;

    type Error = Error;

    async fn poll(&mut self, ctx: ServiceTaskContext) -> TaskResult<Self::Output, Self::Error> {
        (self.f)(ctx).await
    }
}

/// Advance a project's state until it's returning `is_done`
pub struct RunUntilDone;

#[async_trait]
impl Task<ServiceTaskContext> for RunUntilDone {
    type Output = ServiceState;

    type Error = Error;

    async fn poll(&mut self, ctx: ServiceTaskContext) -> TaskResult<Self::Output, Self::Error> {
        if !<ServiceState as EndState<ServiceDockerContext>>::is_done(&ctx.state) {
            TaskResult::Pending(ctx.state.next(&ctx.docker_context).await.unwrap())
        } else {
            TaskResult::Done(ctx.state)
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
/// is encountered, the `ServiceTask` completes early passing through
/// the error. The value returned by the inner tasks upon their
/// completion is committed back to persistence through
pub struct ServiceTask<T, D: Dal + Send + Sync + 'static> {
    service_id: Ulid,
    docker_context: ServiceDockerContext,
    tasks: VecDeque<T>,
    dal: D,
}

impl<T, D: Dal + Send + Sync + 'static> ServiceTask<T, D> {
    pub fn service_id(&self) -> &Ulid {
        &self.service_id
    }
}

/// A context for tasks which are scoped to a specific service.
///
/// This will be always instantiated with the latest known state of
/// the service and gives access to the broader deployer context.
#[derive(Clone)]
pub struct ServiceTaskContext {
    /// The id of the service this taks is about
    pub service_id: Ulid,
    /// The last known state of the project
    pub state: ServiceState,
    /// Service docker context
    pub docker_context: ServiceDockerContext,
}

pub type BoxedTask<Ctx = (), O = ()> = Box<dyn Task<Ctx, Output = O, Error = Error>>;

#[async_trait]
impl<T, D: Dal + Sync + 'static> Task<()> for ServiceTask<T, D>
where
    T: Task<ServiceTaskContext, Output = ServiceState, Error = Error>,
{
    type Output = ();

    type Error = Error;

    async fn poll(&mut self, _: ()) -> TaskResult<Self::Output, Self::Error> {
        if self.tasks.is_empty() {
            return TaskResult::Done(());
        }

        let service = match self.dal.service(&self.service_id).await {
            Ok(inner) => inner,
            Err(err) => return TaskResult::Err(Error::Dal(err)),
        };

        let service_task_ctx = ServiceTaskContext {
            service_id: self.service_id.clone(),
            state: service.state,
            docker_context: self.docker_context.clone(),
        };

        let span = info_span!(
            "polling service",
            ctx.service_id = ?service_task_ctx.service_id,
            ctx.state = service_task_ctx.state.state()
        );
        let _ = span.enter();

        let task = self.tasks.front_mut().unwrap();

        let timeout = sleep(SERVICE_TASK_MAX_IDLE_TIMEOUT);
        let res = {
            let mut poll = task.poll(service_task_ctx);
            tokio::select! {
                res = &mut poll => res,
                _ = timeout => {
                    warn!(
                        service_id = ?self.service_id,
                        "a task has been idling for a long time"
                    );
                    poll.await
                }
            }
        };

        if let Some(update) = res.as_ref().ok() {
            trace!(new_state = ?update.state(), "new state");
            match self
                .dal
                .update_service_state(self.service_id.clone(), update.clone())
                .await
            {
                Ok(_) => trace!(new_state = ?update.state(), "successfully updated project state"),
                Err(err) => {
                    error!(err = %err, "could not update project state");
                    return TaskResult::Err(Error::Dal(err));
                }
            };
        }

        trace!(result = res.to_str(), "poll result");

        match res {
            TaskResult::Pending(_) => TaskResult::Pending(()),
            TaskResult::TryAgain => TaskResult::TryAgain,
            TaskResult::Done(_) => {
                let _ = self.tasks.pop_front().unwrap();
                if self.tasks.is_empty() {
                    return TaskResult::Done(());
                }

                TaskResult::Pending(())
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

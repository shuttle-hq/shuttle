use std::fmt::Debug;

use tokio::sync::mpsc::{channel, Receiver, Sender};
use tracing::info;

use crate::project::Project;
use crate::{AccountName, Context, EndState, Error, ProjectName, Service, State};

#[derive(Debug, Clone)]
pub struct Work<W = Project> {
    pub project_name: ProjectName,
    pub account_name: AccountName,
    pub work: W,
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
        let _ = self.send.take();

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

#[cfg(test)]
pub mod tests {
    use std::convert::Infallible;

    use anyhow::anyhow;

    use super::*;
    use crate::tests::{World, WorldContext};

    pub struct DummyService<S> {
        world: World,
        state: Option<S>,
    }

    impl DummyService<()> {
        pub async fn new<S>() -> DummyService<S> {
            let world = World::new().await;
            DummyService { world, state: None }
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

        async fn next<C: Context<'c>>(mut self, _ctx: &C) -> Result<Self::Next, Self::Error> {
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
        let svc = DummyService::new::<FiniteState>().await;

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
}

use std::error::Error as StdError;
use std::{convert::Infallible, pin::Pin};

use async_trait::async_trait;
use futures::{stream, Stream};

/// A generic state which can, when provided with a [`Context`], do
/// some work and advance itself
#[async_trait]
pub trait State<Ctx>: Send {
    type Next;

    type Error;

    async fn next(self, ctx: &Ctx) -> Result<Self::Next, Self::Error>;
}

pub type StateTryStream<'c, St, Err> = Pin<Box<dyn Stream<Item = Result<St, Err>> + Send + 'c>>;

pub trait EndState<Ctx>
where
    Self: State<Ctx, Error = Infallible, Next = Self>,
{
    fn is_done(&self) -> bool;
}

pub trait EndStateExt<Ctx>: TryState + EndState<Ctx>
where
    Ctx: Sync,
    Self: Clone,
{
    /// Convert the state into a [`TryStream`] that yields
    /// the generated states.
    ///
    /// This stream will not end.
    fn into_stream<'c>(self, ctx: &'c Ctx) -> StateTryStream<'c, Self, Self::ErrorVariant>
    where
        Self: 'c,
    {
        Box::pin(stream::try_unfold((self, ctx), |(state, ctx)| async move {
            state
                .next(ctx)
                .await
                .unwrap() // EndState's `next` is Infallible
                .into_result()
                .map(|state| Some((state.clone(), (state, ctx))))
        }))
    }
}

impl<Ctx, S> EndStateExt<Ctx> for S
where
    S: Clone + TryState + EndState<Ctx>,
    Ctx: Send + Sync,
{
}

/// A [`State`] which contains all its transitions, including
/// failures
pub trait TryState: Sized {
    type ErrorVariant;

    fn into_result(self) -> Result<Self, Self::ErrorVariant>;
}

pub trait IntoTryState<S>
where
    S: TryState,
{
    fn into_try_state(self) -> Result<S, Infallible>;
}

impl<S, F, Err> IntoTryState<S> for Result<F, Err>
where
    S: TryState + From<F> + From<Err>,
{
    fn into_try_state(self) -> Result<S, Infallible> {
        self.map(|s| S::from(s)).or_else(|err| Ok(S::from(err)))
    }
}

#[async_trait]
pub trait Refresh<Ctx>: Sized {
    type Error: StdError;

    async fn refresh(self, ctx: &Ctx) -> Result<Self, Self::Error>;
}

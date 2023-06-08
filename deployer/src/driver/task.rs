use axum::async_trait;

use crate::error::Error;

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

pub type BoxedTask<Ctx = (), O = ()> = Box<dyn Task<Ctx, Output = O, Error = Error>>;

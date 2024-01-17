#![doc = include_str!("../README.md")]
#[cfg(feature = "poise")]
use poise::serenity_prelude as serenity;
use std::net::SocketAddr;
#[cfg(feature = "poise-0-5")]
use std::sync::Arc;

/// A wrapper type for [poise::Framework] so we can implement [shuttle_runtime::Service] for it.
#[cfg(feature = "poise")]
pub struct PoiseService<T, E> {
    pub framework: poise::Framework<T, E>,
    pub token: String,
    pub intents: serenity::GatewayIntents,
}
/// A wrapper type for [poise::Framework] so we can implement [shuttle_runtime::Service] for it.
#[cfg(feature = "poise-0-5")]
pub struct PoiseService<T, E>(pub Arc<poise_0_5::Framework<T, E>>);

#[shuttle_runtime::async_trait]
impl<T, E> shuttle_runtime::Service for PoiseService<T, E>
where
    T: Send + Sync + 'static,
    E: Send + Sync + 'static,
{
    async fn bind(mut self, _addr: SocketAddr) -> Result<(), shuttle_runtime::Error> {
        #[cfg(feature = "poise")]
        serenity::ClientBuilder::new(self.token, self.intents)
            .framework(self.framework)
            .await
            .map_err(shuttle_runtime::CustomError::new)?
            .start_autosharded()
            .await
            .map_err(shuttle_runtime::CustomError::new)?;

        #[cfg(feature = "poise-0-5")]
        self.0
            .start_autosharded()
            .await
            .map_err(shuttle_runtime::CustomError::new)?;

        Ok(())
    }
}

#[cfg(feature = "poise")]
impl<T, E> From<(poise::Framework<T, E>, String, serenity::GatewayIntents)> for PoiseService<T, E> {
    fn from(
        (framework, token, intents): (poise::Framework<T, E>, String, serenity::GatewayIntents),
    ) -> Self {
        Self {
            framework,
            token,
            intents,
        }
    }
}

#[cfg(feature = "poise-0-5")]
impl<T, E> From<Arc<poise_0_5::Framework<T, E>>> for PoiseService<T, E> {
    fn from(framework: Arc<poise_0_5::Framework<T, E>>) -> Self {
        Self(framework)
    }
}

#[doc = include_str!("../README.md")]
pub type ShuttlePoise<T, E> = Result<PoiseService<T, E>, shuttle_runtime::Error>;

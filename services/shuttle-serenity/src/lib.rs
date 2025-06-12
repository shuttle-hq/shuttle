#![doc = include_str!("../README.md")]
use std::net::SocketAddr;

#[cfg(feature = "serenity")]
use serenity::Client;
#[cfg(feature = "serenity-0-11")]
use serenity_0_11::Client;

#[cfg(feature = "serenity")]
pub use serenity;
#[cfg(feature = "serenity-0-11")]
pub use serenity_0_11 as serenity;

/// A wrapper type for [serenity::Client] so we can implement [shuttle_runtime::Service] for it.
pub struct SerenityService(pub Client);

#[shuttle_runtime::async_trait]
impl shuttle_runtime::Service for SerenityService {
    async fn bind(mut self, _addr: SocketAddr) -> Result<(), shuttle_runtime::BoxDynError> {
        self.0.start_autosharded().await?;

        Ok(())
    }
}

impl From<Client> for SerenityService {
    fn from(router: Client) -> Self {
        Self(router)
    }
}

#[doc = include_str!("../README.md")]
pub type ShuttleSerenity = Result<SerenityService, shuttle_runtime::BoxDynError>;

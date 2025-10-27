use anyhow::Result;

use crate::impulse::{
    args::{LoginArgs, LogoutArgs},
    Impulse, ImpulseCommandOutput,
};

impl Impulse {
    pub async fn login(&self, _login_args: LoginArgs) -> Result<ImpulseCommandOutput> {
        unimplemented!();
        Ok(ImpulseCommandOutput::None)
    }

    pub async fn logout(&self, _logout_args: LogoutArgs) -> Result<ImpulseCommandOutput> {
        unimplemented!();
        Ok(ImpulseCommandOutput::None)
    }
}

use serde::{Deserialize, Serialize};

use crate::{
    constants::limits::{MAX_PROJECTS_DEFAULT, MAX_PROJECTS_EXTRA},
    models::user::AccountTier,
};

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct Limits {
    /// The amount of projects this user can create.
    pub project_limit: u32,
    /// Whether this user has permission to provision RDS instances.
    #[deprecated(
        since = "0.38.0",
        note = "This was replaced with rds_quota, but old runtimes might still try to deserialize a claim expecting this field"
    )]
    #[serde(skip_deserializing)]
    rds_access: bool,
    /// The quantity of RDS instances this user can provision.
    pub rds_quota: u32,
}

impl Default for Limits {
    fn default() -> Self {
        #[allow(deprecated)]
        Self {
            project_limit: MAX_PROJECTS_DEFAULT,
            rds_access: false,
            rds_quota: 0,
        }
    }
}

impl Limits {
    pub fn new(project_limit: u32, rds_quota: u32) -> Self {
        #[allow(deprecated)]
        Self {
            project_limit,
            rds_access: false,
            rds_quota,
        }
    }

    pub fn project_limit(&self) -> u32 {
        self.project_limit
    }

    /// Use the subscription quantity to set the RDS quota for this claim.
    pub fn set_rds_quota(&mut self, quantity: u32) {
        self.rds_quota = quantity;
    }

    /// Get the current RDS limits
    pub fn rds_quota(&self) -> u32 {
        self.rds_quota
    }
}

impl From<AccountTier> for Limits {
    fn from(value: AccountTier) -> Self {
        match value {
            AccountTier::Admin
            | AccountTier::Basic
            | AccountTier::PendingPaymentPro
            | AccountTier::Deployer => Self::default(),
            AccountTier::Pro | AccountTier::CancelledPro | AccountTier::Team => {
                Self::new(MAX_PROJECTS_EXTRA, 1)
            }
        }
    }
}

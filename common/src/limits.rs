use serde::{Deserialize, Serialize};

use crate::{
    claims::{AccountTier, Claim, Scope},
    constants::limits::{MAX_PROJECTS_DEFAULT, MAX_PROJECTS_EXTRA},
};

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct Limits {
    /// The amount of projects this user can create.
    project_limit: u32,
    /// Whether this user has permission to provision RDS instances.
    #[deprecated(
        since = "0.38.0",
        note = "This was replaced with rds_quota, but old runtimes might still try to deserialize a claim expecting this field"
    )]
    #[serde(skip_deserializing)]
    rds_access: bool,
    /// The quantity of RDS instances this user can provision.
    rds_quota: u32,
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
    pub fn rds_quota(&mut self, quantity: u32) {
        self.rds_quota = quantity;
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

pub trait ClaimExt {
    /// Verify that the [Claim] has the [Scope::Admin] scope.
    fn is_admin(&self) -> bool;
    /// Verify that the user's current project count is lower than the account limit in [Claim::limits].
    fn can_create_project(&self, current_count: u32) -> bool;
    /// Verify that the user has permission to provision RDS instances.
    fn can_provision_rds(&self) -> bool;
}

impl ClaimExt for Claim {
    fn is_admin(&self) -> bool {
        self.scopes.contains(&Scope::Admin)
    }

    fn can_create_project(&self, current_count: u32) -> bool {
        self.is_admin() || self.limits.project_limit() > current_count
    }

    fn can_provision_rds(&self) -> bool {
        self.is_admin() || self.limits.rds_quota > 0
    }
}

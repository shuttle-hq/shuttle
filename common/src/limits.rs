use serde::{Deserialize, Serialize};

use crate::{
    claims::{AccountTier, Claim, Scope},
    constants::limits::{MAX_PROJECTS_DEFAULT, MAX_PROJECTS_EXTRA},
};

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct Limits {
    /// The amount of projects this user can create.
    project_limit: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            project_limit: MAX_PROJECTS_DEFAULT,
        }
    }
}

impl Limits {
    pub fn new(project_limit: u32) -> Self {
        Self { project_limit }
    }

    pub fn project_limit(&self) -> u32 {
        self.project_limit
    }
}

impl From<AccountTier> for Limits {
    fn from(value: AccountTier) -> Self {
        match value {
            AccountTier::Admin
            | AccountTier::Basic
            | AccountTier::PendingPaymentPro
            | AccountTier::Deployer => Self::default(),
            AccountTier::Pro | AccountTier::Team => Self::new(MAX_PROJECTS_EXTRA),
        }
    }
}

pub trait ClaimExt {
    /// Verify that the [Claim] has the [Scope::Admin] scope.
    fn is_admin(&self) -> bool;
    /// Verify that the user's current project count is lower than the account limit in [Claim::limits].
    fn can_create_project(&self, current_count: u32) -> bool;
}

impl ClaimExt for Claim {
    fn is_admin(&self) -> bool {
        self.scopes.contains(&Scope::Admin)
    }

    fn can_create_project(&self, current_count: u32) -> bool {
        self.is_admin() || self.limits.project_limit() > current_count
    }
}

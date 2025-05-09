use std::collections::HashMap;
#[cfg(feature = "display")]
use std::fmt::Write;

use chrono::{DateTime, NaiveDate, Utc};
#[cfg(feature = "display")]
use crossterm::style::Stylize;
use serde::{Deserialize, Serialize};
use strum::{EnumString, IntoStaticStr};

use super::project::ProjectUsageResponse;

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct UserResponse {
    pub id: String,
    /// Auth0 id (deprecated)
    pub name: Option<String>,
    /// Auth0 id
    pub auth0_id: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    // deprecated
    pub key: Option<String>,
    pub account_tier: AccountTier,
    pub subscriptions: Vec<Subscription>,
    pub flags: Option<Vec<String>>,
}

impl UserResponse {
    #[cfg(feature = "display")]
    pub fn to_string_colored(&self) -> String {
        let mut s = String::new();
        writeln!(&mut s, "{}", "Account info:".bold()).unwrap();
        writeln!(&mut s, "  User ID: {}", self.id).unwrap();
        writeln!(
            &mut s,
            "  Account tier: {}",
            self.account_tier.to_string_fancy()
        )
        .unwrap();
        if !self.subscriptions.is_empty() {
            writeln!(&mut s, "  Subscriptions:").unwrap();
            for sub in &self.subscriptions {
                writeln!(
                    &mut s,
                    "    - {}: Type: {}, Quantity: {}, Created: {}, Updated: {}",
                    sub.id, sub.r#type, sub.quantity, sub.created_at, sub.updated_at,
                )
                .unwrap();
            }
        }
        if let Some(flags) = self.flags.as_ref() {
            if !flags.is_empty() {
                writeln!(&mut s, "  Feature flags:").unwrap();
                for flag in flags {
                    writeln!(&mut s, "    - {}", flag).unwrap();
                }
            }
        }

        s
    }
}

#[derive(
    // std
    Clone,
    Debug,
    Default,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    // serde
    Deserialize,
    Serialize,
    // strum
    EnumString,
    IntoStaticStr,
    strum::Display,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub enum AccountTier {
    #[default]
    Basic,
    /// Partial access to Pro features and higher limits than Basic
    ProTrial,
    /// A Basic user that is pending a payment to go back to Pro
    // soft-deprecated
    PendingPaymentPro,
    /// Pro user with an expiring subscription
    // soft-deprecated
    CancelledPro,
    Pro,
    Growth,
    /// Growth tier but even higher limits
    Employee,
    /// No limits, full API access, admin endpoint access
    Admin,

    /// Forward compatibility
    #[cfg(feature = "unknown-variants")]
    #[doc(hidden)]
    #[typeshare(skip)]
    #[serde(untagged, skip_serializing)]
    #[strum(default, to_string = "Unknown: {0}")]
    Unknown(String),
}
impl AccountTier {
    pub fn to_string_fancy(&self) -> String {
        match self {
            Self::Basic => "Community".to_owned(),
            Self::ProTrial => "Pro Trial".to_owned(),
            Self::PendingPaymentPro => "Community (pending payment for Pro)".to_owned(),
            Self::CancelledPro => "Pro (subscription cancelled)".to_owned(),
            Self::Pro => "Pro".to_owned(),
            Self::Growth => "Growth".to_owned(),
            Self::Employee => "Employee".to_owned(),
            Self::Admin => "Admin".to_owned(),
            #[cfg(feature = "unknown-variants")]
            Self::Unknown(_) => self.to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct Subscription {
    pub id: String,
    pub r#type: SubscriptionType,
    pub quantity: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct SubscriptionRequest {
    pub id: String,
    pub r#type: SubscriptionType,
    pub quantity: i32,
}

#[derive(
    // std
    Clone,
    Debug,
    Eq,
    PartialEq,
    // serde
    Deserialize,
    Serialize,
    // strum
    EnumString,
    strum::Display,
    IntoStaticStr,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub enum SubscriptionType {
    Pro,
    Rds,

    /// Forward compatibility
    #[cfg(feature = "unknown-variants")]
    #[doc(hidden)]
    #[typeshare(skip)]
    #[serde(untagged, skip_serializing)]
    #[strum(default, to_string = "Unknown: {0}")]
    Unknown(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deser() {
        assert_eq!(
            serde_json::from_str::<AccountTier>("\"basic\"").unwrap(),
            AccountTier::Basic
        );
    }
    #[cfg(feature = "unknown-variants")]
    #[test]
    fn unknown_deser() {
        assert_eq!(
            serde_json::from_str::<AccountTier>("\"\"").unwrap(),
            AccountTier::Unknown("".to_string())
        );
        assert_eq!(
            serde_json::from_str::<AccountTier>("\"hisshiss\"").unwrap(),
            AccountTier::Unknown("hisshiss".to_string())
        );
        assert!(serde_json::to_string(&AccountTier::Unknown("asdf".to_string())).is_err());
    }
    #[cfg(not(feature = "unknown-variants"))]
    #[test]
    fn not_unknown_deser() {
        assert!(serde_json::from_str::<AccountTier>("\"\"").is_err());
        assert!(serde_json::from_str::<AccountTier>("\"hisshiss\"").is_err());
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct CreateAccountRequest {
    pub auth0_id: String,
    pub account_tier: AccountTier,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct UpdateAccountTierRequest {
    pub account_tier: AccountTier,
}

/// Response for the /user/me/usage backend endpoint
#[derive(Debug, Default, Deserialize, Serialize, Clone)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct UserUsageResponse {
    /// Billing cycle start, or monthly from user creation
    /// depending on the account tier
    pub start: NaiveDate,

    /// Billing cycle end, or end of month from user creation
    /// depending on the account tier
    pub end: NaiveDate,

    /// HashMap of project related metrics for this cycle
    /// keyed by project_id
    pub projects: HashMap<String, ProjectUsageResponse>,
}

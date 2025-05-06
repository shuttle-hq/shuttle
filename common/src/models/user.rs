#[cfg(feature = "display")]
use std::fmt::Write;

use chrono::{DateTime, Utc};
#[cfg(feature = "display")]
use crossterm::style::Stylize;
use serde::{Deserialize, Serialize};
use strum::{EnumString, IntoStaticStr};

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[typeshare::typeshare]
pub struct UserResponse {
    pub id: String,
    /// Auth0 id (deprecated)
    pub name: Option<String>,
    /// Auth0 id
    pub auth0_id: Option<String>,
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
            self.account_tier.as_str_fancy()
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
    Copy,
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
}
impl AccountTier {
    pub const fn as_str_fancy(&self) -> &'static str {
        match self {
            Self::Basic => "Community",
            Self::ProTrial => "Pro Trial",
            Self::PendingPaymentPro => "Community (pending payment for Pro)",
            Self::CancelledPro => "Pro (subscription cancelled)",
            Self::Pro => "Pro",
            Self::Growth => "Growth",
            Self::Employee => "Employee",
            Self::Admin => "Admin",
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
}

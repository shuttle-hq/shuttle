#[cfg(feature = "display")]
use std::fmt::Write;

use chrono::{DateTime, Utc};
#[cfg(feature = "display")]
use crossterm::style::Stylize;
use serde::{Deserialize, Serialize};
use strum::{EnumString, IntoStaticStr};

#[derive(Debug, Deserialize, Serialize)]
#[typeshare::typeshare]
pub struct UserResponse {
    pub name: String,
    pub id: String,
    pub key: String,
    pub account_tier: AccountTier,
    pub subscriptions: Vec<Subscription>,
    pub has_access_to_beta: Option<bool>,
}

impl UserResponse {
    #[cfg(feature = "display")]
    pub fn to_string_colored(&self) -> String {
        let mut s = String::new();
        writeln!(&mut s, "{}", "Account info:".bold()).unwrap();
        writeln!(&mut s, "  User Id: {}", self.id).unwrap();
        writeln!(&mut s, "  Username: {}", self.name).unwrap();
        writeln!(&mut s, "  Account tier: {}", self.account_tier).unwrap();
        writeln!(&mut s, "  Subscriptions:").unwrap();
        for sub in &self.subscriptions {
            writeln!(
                &mut s,
                "    - {}: Type: {}, Quantity: {}, Created: {}, Updated: {}",
                sub.id, sub.r#type, sub.quantity, sub.created_at, sub.updated_at,
            )
            .unwrap();
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
#[typeshare::typeshare]
pub enum AccountTier {
    #[default]
    Basic,
    /// A basic user that is pending a payment on the backend
    PendingPaymentPro,
    CancelledPro,
    Pro,
    Team,
    /// Higher limits and partial admin endpoint access
    Employee,
    /// Unlimited resources, full API access, admin endpoint access
    Admin,
    Deployer,
}

#[derive(Debug, Deserialize, Serialize)]
#[typeshare::typeshare]
pub struct Subscription {
    pub id: String,
    pub r#type: SubscriptionType,
    pub quantity: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
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
#[typeshare::typeshare]
pub enum SubscriptionType {
    Pro,
    Rds,
}

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// In normal cases, a string with the format `user_<ULID>`.
/// This is a soft rule and the string can be something different.
pub type UserId = String;

#[derive(Deserialize, Serialize, Debug)]
#[typeshare::typeshare]
pub struct UserResponse {
    pub name: String,
    pub id: String,
    pub key: String,
    pub account_tier: String,
    pub subscriptions: Vec<Subscription>,
    pub has_access_to_beta: bool,
}

#[derive(Deserialize, Serialize, Debug)]
#[typeshare::typeshare]
pub struct Subscription {
    pub id: String,
    pub r#type: SubscriptionType,
    pub quantity: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Deserialize, Debug)]
#[typeshare::typeshare]
pub struct SubscriptionRequest {
    pub id: String,
    pub r#type: SubscriptionType,
    pub quantity: i32,
}

#[derive(Clone, Debug, EnumString, Display, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[typeshare::typeshare]
pub enum SubscriptionType {
    Pro,
    Rds,
}

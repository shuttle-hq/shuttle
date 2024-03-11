use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// In normal cases, a string with the format `user_<ULID>`.
/// This is a soft rule and the string can be something different.
pub type UserId = String;

#[derive(Deserialize, Serialize, Debug)]
pub struct Response {
    pub name: String,
    pub id: UserId,
    pub key: String,
    pub account_tier: String,
    pub subscriptions: Vec<Subscription>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Subscription {
    pub id: String,
    pub r#type: SubscriptionType,
    pub quantity: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Deserialize, Debug)]
pub struct SubscriptionRequest {
    pub id: String,
    pub r#type: SubscriptionType,
    pub quantity: i32,
}

#[derive(Clone, Debug, EnumString, Display, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum SubscriptionType {
    Pro,
    Rds,
}

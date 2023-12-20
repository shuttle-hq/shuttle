use serde::{Deserialize, Serialize};

/// Used when sending requests to the Auth service to add a new item to a user's subscription.
#[derive(Debug, Deserialize, Serialize)]
pub struct NewSubscriptionItem {
    pub item: SubscriptionItem,
    pub quantity: u64,
}

impl NewSubscriptionItem {
    pub fn new(item: SubscriptionItem, quantity: u64) -> NewSubscriptionItem {
        NewSubscriptionItem { item, quantity }
    }
}

#[derive(Deserialize, Debug, Serialize)]
pub enum SubscriptionItem {
    AwsRds,
}

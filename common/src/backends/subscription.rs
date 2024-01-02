use serde::{Deserialize, Serialize};

/// Used when sending requests to the Auth service to add a new item to a user's subscription.
#[derive(Debug, Deserialize, Serialize)]
pub struct NewSubscriptionItem {
    // A unique id to tie the subscription item to a resource, e.g. a database name or resource ulid.
    pub id: String,
    pub r#type: SubscriptionItemType,
    pub quantity: u64,
}

impl NewSubscriptionItem {
    pub fn new(
        id: impl ToString,
        item: SubscriptionItemType,
        quantity: u64,
    ) -> NewSubscriptionItem {
        NewSubscriptionItem {
            id: id.to_string(),
            r#type: item,
            quantity,
        }
    }
}

#[derive(Deserialize, Debug, Serialize)]
pub enum SubscriptionItemType {
    AwsRds,
}

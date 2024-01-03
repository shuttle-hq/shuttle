use serde::{Deserialize, Serialize};

/// Used when sending requests to the Auth service to add a new item to a user's subscription.
#[derive(Debug, Deserialize, Serialize)]
pub struct NewSubscriptionItem {
    /// A unique id to tie the subscription item to a resource, e.g. a database name or resource ulid,
    /// that will be inserted into the metadata of the new Stripe subscription item.
    pub metadata_id: String,
    pub r#type: SubscriptionItemType,
    pub quantity: u64,
}

impl NewSubscriptionItem {
    pub fn new(
        metadata_id: impl ToString,
        item: SubscriptionItemType,
        quantity: u64,
    ) -> NewSubscriptionItem {
        NewSubscriptionItem {
            metadata_id: metadata_id.to_string(),
            r#type: item,
            quantity,
        }
    }
}

#[derive(Deserialize, Debug, Serialize)]
pub enum SubscriptionItemType {
    AwsRds,
}

use serde::{Deserialize, Serialize};

use crate::constants::stripe_price_ids::AWS_RDS_INSTANCE_RECURRING;

/// Used when sending requests to the Auth service to add a new item to a user's subscription.
#[derive(Debug, Deserialize, Serialize)]
pub struct SubscriptionItem {
    price_id: String,
    quantity: u64,
}

impl SubscriptionItem {
    pub fn new(price_id: PriceId, quantity: u64) -> SubscriptionItem {
        SubscriptionItem {
            price_id: price_id.value().to_string(),
            quantity,
        }
    }

    pub fn price_id(&self) -> String {
        self.price_id.clone()
    }
    pub fn quantity(&self) -> u64 {
        self.quantity
    }
}

/// Each variant of this enum should be associated with a constant in
/// [stripe_price_ids](crate::constants::stripe_price_ids), which will be returned by the
/// value method.
pub enum PriceId {
    AwsRdsRecurring,
}

impl PriceId {
    /// Return the const price ID associated with the
    fn value(&self) -> &'static str {
        match self {
            PriceId::AwsRdsRecurring => AWS_RDS_INSTANCE_RECURRING,
        }
    }
}

use serde::Deserialize;

use crate::constants::stripe_price_ids::AWS_RDS_INSTANCE_RECURRING;

#[derive(Deserialize, Debug)]
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

pub enum PriceId {
    AwsRdsRecurring,
}

impl PriceId {
    fn value(&self) -> &'static str {
        match self {
            PriceId::AwsRdsRecurring => AWS_RDS_INSTANCE_RECURRING,
        }
    }
}

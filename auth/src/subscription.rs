pub mod stripe_price_ids {
    /// The price ID of the recurring AWS RDS instance product.
    /// TODO: this is set to the id of the test product, set to real product before releasing.
    pub const AWS_RDS_INSTANCE_RECURRING: &str = "price_1OIS06FrN7EDaGOjaV0GXD7P";
}

pub trait SubscriptionItemExt {
    fn price_id(&self) -> String;
}

impl SubscriptionItemExt for shuttle_common::backends::subscription::SubscriptionItem {
    fn price_id(&self) -> String {
        match self {
            shuttle_common::backends::subscription::SubscriptionItem::AwsRds => {
                stripe_price_ids::AWS_RDS_INSTANCE_RECURRING.to_string()
            }
        }
    }
}

use shuttle_common::backends::subscription::SubscriptionItem;

pub mod stripe_price_ids {
    /// TODO: this is set to the id of the test product, set to real product before releasing.
    pub const PRODUCTION_AWS_RDS_INSTANCE_RECURRING: &str = "TODO: change to prod ID";
    /// The price ID of the recurring AWS RDS instance product on staging.
    pub const STAGING_AWS_RDS_INSTANCE_RECURRING: &str = "price_1OIS06FrN7EDaGOjaV0GXD7P";
}

pub trait SubscriptionItemExt {
    fn price_id(&self) -> String;
}

impl SubscriptionItemExt for SubscriptionItem {
    fn price_id(&self) -> String {
        let is_production = std::env::var("SHUTTLE_ENV").is_ok_and(|env| env == "production");

        if is_production {
            match self {
                SubscriptionItem::AwsRds => {
                    stripe_price_ids::PRODUCTION_AWS_RDS_INSTANCE_RECURRING.to_string()
                }
            }
        } else {
            match self {
                SubscriptionItem::AwsRds => {
                    stripe_price_ids::STAGING_AWS_RDS_INSTANCE_RECURRING.to_string()
                }
            }
        }
    }
}

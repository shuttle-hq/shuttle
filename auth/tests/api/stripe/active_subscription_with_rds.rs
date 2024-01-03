pub(crate) const MOCKED_ACTIVE_SUBSCRIPTION_WITH_RDS: &str = r#"{
    "id": "sub_1Nw8xOD8t1tt0S3DtwAuOVp6",
    "object": "subscription",
    "application": null,
    "application_fee_percent": null,
    "automatic_tax": {
      "enabled": false
    },
    "billing_cycle_anchor": 1696102566,
    "billing_thresholds": null,
    "cancel_at": null,
    "cancel_at_period_end": false,
    "canceled_at": null,
    "cancellation_details": {
      "comment": null,
      "feedback": null,
      "reason": null
    },
    "collection_method": "charge_automatically",
    "created": 1696102566,
    "currency": "ron",
    "current_period_end": 1698694566,
    "current_period_start": 1696102566,
    "customer": "cus_OjcBtb9CGkRN0Q",
    "days_until_due": null,
    "default_payment_method": "pm_1Nw8xND8t1tt0S3DdoPw8WzZ",
    "default_source": null,
    "default_tax_rates": [],
    "description": null,
    "discount": null,
    "ended_at": null,
    "items": {
      "object": "list",
      "data": [
        {
          "id": "si_OjcB0PrsQ861FB",
          "object": "subscription_item",
          "billing_thresholds": null,
          "created": 1696102567,
          "metadata": {},
          "plan": {
            "id": "price_1NvdmxD8t1tt0S3DBi2jTI92",
            "object": "plan",
            "active": true,
            "aggregate_usage": null,
            "amount": 10000,
            "amount_decimal": "10000",
            "billing_scheme": "per_unit",
            "created": 1695982755,
            "currency": "ron",
            "interval": "month",
            "interval_count": 1,
            "livemode": false,
            "metadata": {},
            "nickname": null,
            "product": "prod_Oj5yfmphYbZ8RE",
            "tiers_mode": null,
            "transform_usage": null,
            "trial_period_days": null,
            "usage_type": "licensed"
          },
          "price": {
            "id": "price_1NvdmxD8t1tt0S3DBi2jTI92",
            "object": "price",
            "active": true,
            "billing_scheme": "per_unit",
            "created": 1695982755,
            "currency": "ron",
            "custom_unit_amount": null,
            "livemode": false,
            "lookup_key": null,
            "metadata": {},
            "nickname": null,
            "product": "prod_Oj5yfmphYbZ8RE",
            "recurring": {
              "aggregate_usage": null,
              "interval": "month",
              "interval_count": 1,
              "trial_period_days": null,
              "usage_type": "licensed"
            },
            "tax_behavior": "unspecified",
            "tiers_mode": null,
            "transform_quantity": null,
            "type": "recurring",
            "unit_amount": 10000,
            "unit_amount_decimal": "10000"
          },
          "quantity": 1,
          "subscription": "sub_1Nw8xOD8t1tt0S3DtwAuOVp6",
          "tax_rates": []
        },
        {
            "id": "si_PIi5CjfGcXmRKe",
            "object": "subscription_item",
            "billing_thresholds": null,
            "created": 1704196916,
            "metadata": {
              "id": "database-test-db"
            },
            "plan": {
              "id": "price_1OIS06FrN7EDaGOjaV0GXD7P",
              "object": "plan",
              "active": true,
              "aggregate_usage": null,
              "amount": 2000,
              "amount_decimal": "2000",
              "billing_scheme": "per_unit",
              "created": 1701418986,
              "currency": "usd",
              "interval": "month",
              "interval_count": 1,
              "livemode": false,
              "metadata": {
              },
              "nickname": null,
              "product": "prod_P6O33MsTZ6yRCI",
              "tiers_mode": null,
              "transform_usage": null,
              "trial_period_days": null,
              "usage_type": "licensed"
            },
            "price": {
              "id": "price_1OIS06FrN7EDaGOjaV0GXD7P",
              "object": "price",
              "active": true,
              "billing_scheme": "per_unit",
              "created": 1701418986,
              "currency": "usd",
              "custom_unit_amount": null,
              "livemode": false,
              "lookup_key": null,
              "metadata": {
              },
              "nickname": null,
              "product": "prod_P6O33MsTZ6yRCI",
              "recurring": {
                "aggregate_usage": null,
                "interval": "month",
                "interval_count": 1,
                "trial_period_days": null,
                "usage_type": "licensed"
              },
              "tax_behavior": "inclusive",
              "tiers_mode": null,
              "transform_quantity": null,
              "type": "recurring",
              "unit_amount": 2000,
              "unit_amount_decimal": "2000"
            },
            "quantity": 1,
            "subscription": "sub_1Nw8xOD8t1tt0S3DtwAuOVp6",
            "tax_rates": [
            ]
          }
      ],
      "has_more": false,
      "total_count": 2,
      "url": "/v1/subscription_items?subscription=sub_1Nw8xOD8t1tt0S3DtwAuOVp6"
    },
    "latest_invoice": "in_1Nw8xOD8t1tt0S3DU4YDQ8ok",
    "livemode": false,
    "metadata": {},
    "next_pending_invoice_item_invoice": null,
    "on_behalf_of": null,
    "pause_collection": null,
    "payment_settings": {
      "payment_method_options": null,
      "payment_method_types": null,
      "save_default_payment_method": "off"
    },
    "pending_invoice_item_interval": null,
    "pending_setup_intent": null,
    "pending_update": null,
    "plan": {
      "id": "price_1NvdmxD8t1tt0S3DBi2jTI92",
      "object": "plan",
      "active": true,
      "aggregate_usage": null,
      "amount": 10000,
      "amount_decimal": "10000",
      "billing_scheme": "per_unit",
      "created": 1695982755,
      "currency": "ron",
      "interval": "month",
      "interval_count": 1,
      "livemode": false,
      "metadata": {},
      "nickname": null,
      "product": "prod_Oj5yfmphYbZ8RE",
      "tiers_mode": null,
      "transform_usage": null,
      "trial_period_days": null,
      "usage_type": "licensed"
    },
    "quantity": 1,
    "schedule": null,
    "start_date": 1696102566,
    "status": "active",
    "test_clock": null,
    "transfer_data": null,
    "trial_end": null,
    "trial_settings": {
      "end_behavior": {
        "missing_payment_method": "create_invoice"
      }
    },
    "trial_start": null
  }
"#;
// This is a synthetic checkout session. It is used to simplify the code path for downgrading to `PendingPaymentPro` tier
// when user payment is overdue.

pub const MOCKED_OVERDUE_PAYMENT_CHECKOUT_SESSION: &str = r#"{
    "id": "cs_test_a11rHy7qRTwFZuj4lBHso3Frq7CMZheZYcYqNXEFBV4oddxXFLx7bT911p",
    "object": "checkout.session",
    "after_expiration": null,
    "allow_promotion_codes": false,
    "amount_subtotal": 10000,
    "amount_total": 10000,
    "automatic_tax": {
      "enabled": false,
      "status": null
    },
    "billing_address_collection": "auto",
    "cancel_url": "https://stripe.com",
    "client_reference_id": null,
    "consent": null,
    "consent_collection": {
      "promotions": "none",
      "terms_of_service": "none"
    },
    "created": 1696098429,
    "currency": "ron",
    "currency_conversion": null,
    "custom_fields": [],
    "custom_text": {
      "shipping_address": null,
      "submit": null,
      "terms_of_service_acceptance": null
    },
    "customer": null,
    "customer_creation": "if_required",
    "customer_details": null,
    "customer_email": null,
    "expires_at": 1696184829,
    "invoice": null,
    "invoice_creation": null,
    "livemode": false,
    "locale": "auto",
    "metadata": {},
    "mode": "subscription",
    "payment_intent": null,
    "payment_link": "plink_1Nw7sYD8t1tt0S3DHQRms10g",
    "payment_method_collection": "always",
    "payment_method_configuration_details": null,
    "payment_method_options": null,
    "payment_method_types": [
      "card"
    ],
    "payment_status": "unpaid",
    "phone_number_collection": {
      "enabled": false
    },
    "recovered_from": null,
    "setup_intent": null,
    "shipping_address_collection": null,
    "shipping_cost": null,
    "shipping_details": null,
    "shipping_options": [],
    "status": "complete",
    "submit_type": "auto",
    "subscription": "sub_1NwObED8t1tt0S3Dq0IYOEsa",
    "success_url": "https://stripe.com",
    "total_details": {
      "amount_discount": 0,
      "amount_shipping": 0,
      "amount_tax": 0
    },
    "url": "https://checkout.stripe.com/c/pay/cs_test_a11rHy7qRTwFZuj4lBHso3Frq7CMZheZYcYqNXEFBV4oddxXFLx7bT911p#fidkdWxOYHwnPyd1blpxYHZxWjA0S3NhbkhBPXE0cXE1VjZBMm4xSjBpMm9LVEFhczBBVjF8XVx1aTdAVlxiUGlyN0J1d2xjXTU2cXNoNExzbzYwS1VufDZOS0IwV1ZUQ290RjxycXxTVEpjNTVIZnZXdVdkUycpJ2N3amhWYHdzYHcnP3F3cGApJ2lkfGpwcVF8dWAnPyd2bGtiaWBabHFgaCcpJ2BrZGdpYFVpZGZgbWppYWB3dic%2FcXdwYHgl"
  }
"#;

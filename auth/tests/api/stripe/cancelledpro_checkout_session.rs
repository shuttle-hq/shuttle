pub(crate) const MOCKED_CANCELLEDPRO_CHECKOUT_SESSION: &str = r#"{
    "id": "cs_test",
    "object": "checkout.session",
    "after_expiration": null,
    "allow_promotion_codes": null,
    "amount_subtotal": 10000,
    "amount_total": 10000,
    "automatic_tax": {
      "enabled": false,
      "status": null
    },
    "billing_address_collection": null,
    "cancel_url": "https://example.com/cancel",
    "client_reference_id": null,
    "consent": null,
    "consent_collection": null,
    "created": 1696102521,
    "currency": "ron",
    "currency_conversion": null,
    "custom_fields": [],
    "custom_text": {
      "shipping_address": null,
      "submit": null,
      "terms_of_service_acceptance": null
    },
    "customer": "cus_OjcBtb9CGkRN0Q",
    "customer_creation": "always",
    "customer_details": {
      "address": {
        "city": null,
        "country": "RO",
        "line1": null,
        "line2": null,
        "postal_code": null,
        "state": null
      },
      "email": "iulian@shuttle.rs",
      "name": "Iulian Barbu",
      "phone": null,
      "tax_exempt": "none",
      "tax_ids": []
    },
    "customer_email": null,
    "expires_at": 1696188921,
    "invoice": "in_1Nw8xOD8t1tt0S3DU4YDQ8ok",
    "invoice_creation": null,
    "livemode": false,
    "locale": null,
    "metadata": {},
    "mode": "subscription",
    "payment_intent": null,
    "payment_link": null,
    "payment_method_collection": "always",
    "payment_method_configuration_details": null,
    "payment_method_options": null,
    "payment_method_types": [
      "card"
    ],
    "payment_status": "paid",
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
    "submit_type": null,
    "subscription": "sub_123",
    "success_url": "https://example.com/success?session_id={CHECKOUT_SESSION_ID}",
    "total_details": {
      "amount_discount": 0,
      "amount_shipping": 0,
      "amount_tax": 0
    },
    "url": null
  }
"#;

use self::{
    active_subscription::MOCKED_ACTIVE_SUBSCRIPTION,
    completed_checkout_session::MOCKED_COMPLETED_CHECKOUT_SESSION,
    incomplete_checkout_session::MOCKED_INCOMPLETE_CHECKOUT_SESSION,
    overdue_payment_checkout_session::MOCKED_OVERDUE_PAYMENT_CHECKOUT_SESSION,
    past_due_subscription::MOCKED_PAST_DUE_SUBSCRIPTION,
};

mod active_subscription;
mod completed_checkout_session;
mod incomplete_checkout_session;
mod overdue_payment_checkout_session;
mod past_due_subscription;

pub(crate) const MOCKED_SUBSCRIPTIONS: &[&str] =
    &[MOCKED_ACTIVE_SUBSCRIPTION, MOCKED_PAST_DUE_SUBSCRIPTION];

pub(crate) const MOCKED_CHECKOUT_SESSIONS: &[&str] = &[
    MOCKED_COMPLETED_CHECKOUT_SESSION,
    MOCKED_INCOMPLETE_CHECKOUT_SESSION,
    MOCKED_OVERDUE_PAYMENT_CHECKOUT_SESSION,
];

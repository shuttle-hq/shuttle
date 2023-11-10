//! Suggestions to be shown to users encountering errors while using cargo-shuttle.
// TODO: Ideally, the suggestions would be inferred from the status codes returned by
// the gateway in case of requests to it, or errors thrown by the client doing work
// on the users machines. This is a naive way of handling the errors that should suggest
// retrying common commands or reach out on our Discord server in case failures persist.

use crossterm::style::Stylize;
use shuttle_common::constants::SHUTTLE_STATUS_URL;

// --------------------------
// API key related

/// Used when logging out and resetting API key fails
pub fn reset_api_key_failed(err: anyhow::Error) -> anyhow::Error {
    println!();
    println!("{}", "Logging out failed".red());
    println!();
    println!("If trying to log out and reset the API key at the same time fails repeatedly, please check Shuttle status at {SHUTTLE_STATUS_URL} or open a help thread on the Discord server.");
    err
}

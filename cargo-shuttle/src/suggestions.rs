//! Suggestions to be shown to users encountering errors while using cargo-shuttle.
// TODO: Ideally, the suggestions would be inferred from the status codes returned by
// the gateway in case of requests to it, or errors thrown by the client doing work
// on the users machines. This is a naive way of handling the errors that should suggest
// retrying common commands or reach out on our Discord server in case failures persist.

use crossterm::style::Stylize;

// --------------------------
// API key related

/// Used when logging out and resetting API key fails
pub fn reset_api_key_failed(err: anyhow::Error) -> anyhow::Error {
    println!();
    println!("{}", "Logging out failed".red());
    println!();
    println!("If trying to log out and reset the API key at the same time fails repeteadly, please check Shuttle status at https://status.shuttle.rs or open a help thread on the Discord server.");
    err
}

// --------------------------
// Deploy related suggestions

/// Used when the deploy request doesn't succeed.
pub fn deploy_request_failure(err: anyhow::Error) -> anyhow::Error {
    println!();
    println!("{}", "Deploy request failed".red());
    println!();
    println!("Please check your project status and deployments:");
    println!();
    println!("1. cargo shuttle project status");
    println!();
    println!("2. cargo shuttle deployment list");
    println!();
    println!(
        "If deploying fails repeteadly, please try restarting your project before deploying again or contacting the team on the Discord server:"
    );
    println!();
    println!("cargo shuttle project restart");
    err
}

/// Especially used for cases where the deployment fails after the
/// deploy request went through (e.g. following the deployment logs, checking
/// the deployment state).
pub fn deployment_setup_failure(err: anyhow::Error, title: &str) -> anyhow::Error {
    println!();
    println!("{}", title.dark_red());
    println!();
    println!(
        "Please check your project status and if the last deployment is recent and is running:"
    );
    println!();
    println!("1. cargo shuttle project status");
    println!();
    println!("2. cargo shuttle deployment list");
    println!();
    println!("You should be able to get the logs of the deployment by running:");
    println!();
    println!("cargo shuttle logs");
    println!();
    println!("Or follow the logs of the deployment by running:");
    println!();
    println!("cargo shuttle logs --follow");
    println!("If the last deployment is not recent or is not running, please try deploying again  or contacting the team on the Discord server:");
    println!();
    println!("cargo shuttle deploy");
    println!();
    println!("Or restart the project before deploying again:");
    println!();
    println!("cargo shuttle project restart");
    err
}

// ----------------------------
// Get logs related suggestions

/// Used to handle the case of getting the last deployment or getting
/// the logs failed.
pub fn get_logs_failure(err: anyhow::Error, title: &str) -> anyhow::Error {
    println!();
    println!("{}", title.red());
    println!();
    println!("Please check your project status and deployments:");
    println!();
    println!("1. cargo shuttle project status");
    println!();
    println!("2. cargo shuttle deployment list");
    println!();
    println!(
        "If getting the logs fails repeteadly, please try restarting your project before getting the logs again or contacting the team on the Discord server:"
    );
    println!();
    println!("cargo shuttle project restart");
    err
}

// -------------------------------
// Deployments related suggestions

/// Used in case of deployment list request failure.
pub fn get_deployments_list_failure(err: anyhow::Error) -> anyhow::Error {
    println!();
    println!("{}", "Fetching the deployments list failed".red());
    println!();
    println!("Please check your project status:");
    println!();
    println!("cargo shuttle project status");
    println!(
        "If getting the deployment list fails repeteadly, please try restarting your project before getting the deployment list again or contacting the team on the Discord server:"
    );
    println!();
    println!("cargo shuttle project restart");
    err
}

/// Used in case of deployment list request failures.
pub fn get_deployment_status_failure(err: anyhow::Error) -> anyhow::Error {
    println!();
    println!("{}", "Fetching the deployments status failed".red());
    println!();
    println!("Please check your project status:");
    println!();
    println!("cargo shuttle project status");
    println!();
    println!(
        "If getting the deployment state fails repeteadly, please try restarting your project before getting the deployment status again or contacting the team on the Discord server:"
    );
    println!();
    println!("cargo shuttle project restart");
    err
}

pub fn stop_deployment_failure(err: anyhow::Error) -> anyhow::Error {
    println!();
    println!("{}", "Stopping the running deployment failed".red());
    println!();
    println!("Please check your project status and whether you have a running deployment:");
    println!();
    println!("1. cargo shuttle project status");
    println!();
    println!("2. cargo shuttle status");
    println!();
    println!(
        "If stopping the running deployment repeteadly, please try restarting your project before stopping the deployment again or contacting the team on the Discord server:"
    );
    println!();
    println!("cargo shuttle project restart");
    err
}

// -----------------------------
// Service resources suggestions

/// Suggestions in case getting the service resources fails.
pub fn get_service_resources_failure(err: anyhow::Error) -> anyhow::Error {
    println!();
    println!("{}", "Fetching the service resources failed".red());
    println!();
    println!("Please check your project status:");
    println!();
    println!("cargo shuttle project status");
    println!();
    println!(
        "If getting the service resources fails repeteadly, please try restarting your project before getting the resources again or contacting the team on the Discord server:"
    );
    println!();
    println!("cargo shuttle project restart");
    err
}

/// Suggestions in case getting the secrets fails.
pub fn get_secrets_failure(err: anyhow::Error) -> anyhow::Error {
    println!();
    println!("{}", "Fetching the service secrets failed".red());
    println!();
    println!("Please check your project status:");
    println!();
    println!("cargo shuttle project status");
    println!();
    println!(
        "If getting the service secrets fails repeteadly, please try restarting your project before getting the resources again or contacting the team on the Discord server:"
    );
    println!();
    println!("cargo shuttle project restart");
    err
}

// --------------------------
// Project related suggestions

/// Used for suggestions in case project operations fail.
pub fn project_request_failure(
    err: anyhow::Error,
    title: &str,
    show_status_suggestion: bool,
    final_suggestion: &str,
) -> anyhow::Error {
    println!();
    println!("{}", title.red());

    if show_status_suggestion {
        println!();
        println!("Please double-check the project status before retrying:");
        println!();
        println!("cargo shuttle project status");
    }

    println!();
    println!(
        "If {}, please check Shuttle status at https://status.shuttle.rs before contacting the team on the Discord server.",
        final_suggestion
    );
    err
}

/// Used for suggestions in case project restart fails.
pub fn project_restart_failure(err: anyhow::Error) -> anyhow::Error {
    project_request_failure(
        err,
        "Project restart failed",
        true,
        "restarting your project or checking its status fail repeteadly",
    )
}

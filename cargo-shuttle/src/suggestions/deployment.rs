use crossterm::style::Stylize;

/// Used in case of deployment list request failure.
pub fn get_deployments_list_failure(err: anyhow::Error) -> anyhow::Error {
    println!();
    println!("{}", "Fetching the deployments list failed".red());
    println!();
    println!("Please check your project status:");
    println!();
    println!("cargo shuttle project status");
    println!(
        "If getting the deployment list fails repeatedly, please try restarting your project before getting the deployment list again or contacting the team on the Discord server:"
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
        "If getting the deployment state fails repeatedly, please try restarting your project before getting the deployment status again or contacting the team on the Discord server:"
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
        "If stopping the running deployment repeatedly, please try restarting your project before stopping the deployment again or contacting the team on the Discord server:"
    );
    println!();
    println!("cargo shuttle project restart");
    err
}

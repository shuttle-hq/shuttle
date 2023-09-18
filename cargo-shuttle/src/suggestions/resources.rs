use crossterm::style::Stylize;

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

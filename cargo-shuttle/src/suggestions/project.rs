use crossterm::style::Stylize;
use shuttle_common::constants::SHUTTLE_STATUS_URL;

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
        "If {}, please check Shuttle status at {SHUTTLE_STATUS_URL} before contacting the team on the Discord server.",
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
        "restarting your project or checking its status fail repeatedly",
    )
}

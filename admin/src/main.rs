use clap::Parser;
use shuttle_admin::{
    args::{AcmeCommand, Args, Command, StatsCommand},
    client::Client,
    config::get_api_key,
};
use std::{
    collections::{hash_map::RandomState, HashMap},
    fmt::Write,
    fs,
};
use tracing::trace;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    trace!(?args, "starting with args");

    let api_key = get_api_key();
    let client = Client::new(args.api_url.clone(), api_key);

    let res = match args.command {
        Command::Revive => client.revive().await.expect("revive to succeed"),
        Command::Destroy => client.destroy().await.expect("destroy to succeed"),
        Command::Acme(AcmeCommand::CreateAccount { email, acme_server }) => {
            let account = client
                .acme_account_create(&email, acme_server)
                .await
                .expect("to create ACME account");

            let mut res = String::new();
            writeln!(res, "Details of ACME account are as follow. Keep this safe as it will be needed to create certificates in the future").unwrap();
            writeln!(res, "{}", serde_json::to_string_pretty(&account).unwrap()).unwrap();

            res
        }
        Command::Acme(AcmeCommand::RequestCertificate {
            fqdn,
            project,
            credentials,
        }) => {
            let credentials = fs::read_to_string(credentials).expect("to read credentials file");
            let credentials =
                serde_json::from_str(&credentials).expect("to parse content of credentials file");

            client
                .acme_request_certificate(&fqdn, &project, &credentials)
                .await
                .expect("to get a certificate challenge response")
        }
        Command::ProjectNames => {
            let projects = client
                .get_projects()
                .await
                .expect("to get list of projects");

            let projects: HashMap<String, String, RandomState> = HashMap::from_iter(
                projects
                    .into_iter()
                    .map(|project| (project.project_name, project.account_name)),
            );

            let mut res = String::new();

            for (project_name, account_name) in &projects {
                let mut issues = Vec::new();
                let cleaned_name = project_name.to_lowercase();

                // Were there any uppercase characters
                if &cleaned_name != project_name {
                    // Since there were uppercase characters, will the new name clash with any existing projects
                    if let Some(other_account) = projects.get(&cleaned_name) {
                        if other_account == account_name {
                            issues.push(
                                "changing to lower case will clash with same owner".to_string(),
                            );
                        } else {
                            issues.push(format!(
                            "changing to lower case will clash with another owner: {other_account}"
                        ));
                        }
                    }
                }

                let cleaned_underscore = cleaned_name.replace('_', "-");
                // Were there any underscore cleanups
                if cleaned_underscore != cleaned_name {
                    // Since there were underscore cleanups, will the new name clash with any existing projects
                    if let Some(other_account) = projects.get(&cleaned_underscore) {
                        if other_account == account_name {
                            issues
                                .push("cleaning underscore will clash with same owner".to_string());
                        } else {
                            issues.push(format!(
                            "cleaning underscore will clash with another owner: {other_account}"
                        ));
                        }
                    }
                }

                let cleaned_separator_name = cleaned_underscore.trim_matches('-');
                // Were there any dash cleanups
                if cleaned_separator_name != cleaned_underscore {
                    // Since there were dash cleanups, will the new name clash with any existing projects
                    if let Some(other_account) = projects.get(cleaned_separator_name) {
                        if other_account == account_name {
                            issues.push("cleaning dashes will clash with same owner".to_string());
                        } else {
                            issues.push(format!(
                                "cleaning dashes will clash with another owner: {other_account}"
                            ));
                        }
                    }
                }

                // Are reserved words used
                match cleaned_separator_name {
                    "shuttleapp" | "shuttle" => issues.push("is a reserved name".to_string()),
                    _ => {}
                }

                // Is it longer than 63 chars
                if cleaned_separator_name.len() > 63 {
                    issues.push("final name is too long".to_string());
                }

                // Only report of problem projects
                if !issues.is_empty() {
                    writeln!(res, "{project_name}")
                        .expect("to write name of project name having issues");

                    for issue in issues {
                        writeln!(res, "\t- {issue}").expect("to write issue with project name");
                    }

                    writeln!(res).expect("to write a new line");
                }
            }

            res
        }
        Command::Stats(StatsCommand::Load { clear }) => {
            let resp = if clear {
                client.clear_load().await.expect("to delete load stats")
            } else {
                client.get_load().await.expect("to get load stats")
            };

            let has_capacity = if resp.has_capacity { "a" } else { "no" };

            format!(
                "Currently {} builds are running and there is {} capacity for new builds",
                resp.builds_count, has_capacity
            )
        }
    };

    println!("{res}");
}

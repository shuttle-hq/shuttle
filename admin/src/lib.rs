pub mod args;
pub mod client;
pub mod config;

use tracing::trace;

use crate::{
    args::{Args, Command},
    client::Client,
    config::get_api_key,
};

pub async fn run(args: Args) {
    trace!(?args, "starting with args");

    let api_key = get_api_key();
    let client = Client::new(args.api_url.clone(), api_key, args.client_timeout);

    match args.command {
        Command::ChangeProjectOwner { .. } => {
            unimplemented!();
        }
        Command::RenewCerts => {
            let certs = client.get_old_certificates().await.unwrap();
            eprintln!("Starting renewals of {} certs in 5 seconds...", certs.len());
            tokio::time::sleep(tokio::time::Duration::from_millis(5000)).await;
            for (cert_id, subject) in certs {
                println!("--> {cert_id} {subject}");
                println!("{:?}", client.renew_certificate(&cert_id).await);
                // prevent api rate limiting
                tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
            }
        }
        Command::UpdateProjectConfig { project_id, json } => {
            let res = client
                .update_project_config(&project_id, serde_json::from_str(&json).unwrap())
                .await
                .unwrap();
            println!("{res:?}");
        }
        Command::AddFeatureFlag { entity, flag } => {
            client.feature_flag(&entity, &flag, true).await.unwrap();
            println!("Added flag {flag} for {entity}");
        }
        Command::RemoveFeatureFlag { entity, flag } => {
            client.feature_flag(&entity, &flag, false).await.unwrap();
            println!("Removed flag {flag} for {entity}");
        }
        Command::Gc {
            days,
            stop_deployments,
            limit,
        } => {
            let project_ids = client.gc_free_tier(days).await.unwrap();
            gc(client, project_ids, stop_deployments, limit).await;
        }
        Command::GcShuttlings {
            minutes,
            stop_deployments,
            limit,
        } => {
            let project_ids = client.gc_shuttlings(minutes).await.unwrap();
            gc(client, project_ids, stop_deployments, limit).await;
        }
        Command::DeleteUser { user_id } => {
            eprintln!("Deleting user {} in 3 seconds...", user_id);
            tokio::time::sleep(tokio::time::Duration::from_millis(3000)).await;
            let msg = client.delete_user(&user_id).await.unwrap();
            println!("{msg}");
        }
        Command::Everything { query } => {
            let v = client.get_user_everything(&query).await.unwrap();
            println!("{}", serde_json::to_string_pretty(&v).unwrap());
        }
    };
}

async fn gc(client: Client, mut project_ids: Vec<String>, stop_deployments: bool, limit: u32) {
    if !stop_deployments {
        for pid in &project_ids {
            println!("{pid}");
        }
        eprintln!("({} projects)", project_ids.len());
        return;
    }

    project_ids.truncate(limit as usize);
    eprintln!(
        "Starting GC of {} projects in 5 seconds...",
        project_ids.len()
    );
    tokio::time::sleep(tokio::time::Duration::from_millis(5000)).await;
    for pid in project_ids {
        println!("{}", client.inner.stop_service(&pid).await.unwrap());
        // prevent api rate limiting
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }
}

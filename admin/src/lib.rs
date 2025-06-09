pub mod args;
pub mod client;

use shuttle_common::{
    config::{Config, ConfigManager, GlobalConfig, GlobalConfigManager},
    constants::{other_env_api_url, SHUTTLE_API_URL},
};

use crate::{
    args::{Args, Command},
    client::Client,
};

pub async fn run(args: Args) {
    tracing::trace!(?args, "starting with args");

    let api_key = match std::env::var("SHUTTLE_API_KEY") {
        Ok(s) => s,
        Err(_) => {
            let mut global = Config::<_, GlobalConfig>::new(
                GlobalConfigManager::new(args.api_env.clone()).unwrap(),
            );
            let path = global.manager.path();
            tracing::trace!(?path, "looking for config");
            if !global.exists() {
                global.create().unwrap();
            }
            global.open().expect("load global configuration");
            global
                .as_ref()
                .unwrap()
                .api_key
                .clone()
                .expect("api key in config")
        }
    };
    let api_url = args
        .api_url
        // calculate env-specific url if no explicit url given but an env was given
        .or_else(|| args.api_env.as_ref().map(|env| other_env_api_url(env)))
        .unwrap_or_else(|| SHUTTLE_API_URL.to_string());
    let api_url = format!("{api_url}/admin");
    tracing::trace!(?api_url, "");

    let client = Client::new(
        // always in admin mode
        api_url,
        api_key,
        args.client_timeout,
    );

    match args.command {
        Command::ChangeProjectOwner {
            project_id,
            new_user_id,
        } => {
            let res = client
                .update_project_owner(&project_id, new_user_id)
                .await
                .unwrap()
                .into_inner();
            println!("{res:?}");
        }
        Command::AddUserToTeam {
            team_user_id,
            user_id,
        } => {
            let res = client
                .add_team_member(&team_user_id, user_id)
                .await
                .unwrap()
                .into_inner();
            println!("{res:?}");
        }
        Command::RenewCerts => {
            let certs = client.get_old_certificates().await.unwrap().into_inner();
            eprintln!("Starting renewals of {} certs in 5 seconds...", certs.len());
            tokio::time::sleep(tokio::time::Duration::from_millis(5000)).await;
            for (cert_id, subject, acm) in certs {
                println!(
                    "--> {cert_id} {subject} {}",
                    if acm.is_some() { "(ACM)" } else { "" }
                );
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
        Command::UpgradeProjectToLb { project_id } => {
            let res = client.upgrade_project_to_lb(&project_id).await.unwrap();
            println!("{res:#?}");
        }
        Command::UpdateProjectScale {
            project_id,
            compute_tier,
            replicas,
        } => {
            let update_config =
                serde_json::json!({"compute_tier": compute_tier, "replicas": replicas});
            let res = client
                .update_project_scale(&project_id, &update_config)
                .await
                .unwrap();
            println!("{res:#?}");
        }
        Command::GetProjectConfig { project_id } => {
            let res = client.get_project_config(&project_id).await.unwrap();
            println!("{res:#?}");
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
            let project_ids = client.gc_free_tier(days).await.unwrap().into_inner();
            gc(client, project_ids, stop_deployments, limit).await;
        }
        Command::GcShuttlings {
            minutes,
            stop_deployments,
            limit,
        } => {
            let project_ids = client.gc_shuttlings(minutes).await.unwrap().into_inner();
            gc(client, project_ids, stop_deployments, limit).await;
        }
        Command::DeleteUser { user_id } => {
            eprintln!("Deleting user {} in 3 seconds...", user_id);
            tokio::time::sleep(tokio::time::Duration::from_millis(3000)).await;
            let msg = client.delete_user(&user_id).await.unwrap();
            println!("{msg}");
        }
        Command::SetAccountTier { user_id, tier } => {
            client.set_user_tier(&user_id, tier.clone()).await.unwrap();
            println!("Set {user_id} to {tier}");
        }
        Command::Everything { query } => {
            let v = client
                .get_user_everything(&query)
                .await
                .unwrap()
                .into_inner();
            println!("{}", serde_json::to_string_pretty(&v).unwrap());
        }
        Command::DowngradeProTrials => {
            let users = client.get_expired_protrials().await.unwrap().into_inner();
            eprintln!(
                "Starting downgrade of {} users in 5 seconds...",
                users.len()
            );
            tokio::time::sleep(tokio::time::Duration::from_millis(5000)).await;
            for user_id in users {
                println!("{user_id}");
                println!("  {:?}", client.downgrade_protrial(&user_id).await);
                // prevent api rate limiting
                tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
            }
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
        println!(
            "{}",
            client.inner.stop_service(&pid).await.unwrap().into_inner()
        );
        // prevent api rate limiting
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }
}

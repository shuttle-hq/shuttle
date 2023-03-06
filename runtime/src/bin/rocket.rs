// The few line below is what we should now codegen for legacy
#[tokio::main]
async fn main() {
    shuttle_runtime::start(loader).await;
}

async fn loader<S: shuttle_common::storage_manager::StorageManager>(
    mut factory: shuttle_runtime::ProvisionerFactory<S>,
    logger: shuttle_service::Logger,
) -> shuttle_service::ShuttleRocket {
    use shuttle_service::ResourceBuilder;

    let secrets = shuttle_secrets::Secrets::new().build(&mut factory).await?;

    rocket(secrets).await
}

// Everything below this is the usual code a user will write
use anyhow::anyhow;
use rocket::response::status::BadRequest;
use rocket::State;
use shuttle_secrets::SecretStore;

#[rocket::get("/secret")]
async fn secret(state: &State<MyState>) -> Result<String, BadRequest<String>> {
    Ok(state.secret.clone())
}

struct MyState {
    secret: String,
}

// #[shuttle_service::main]
pub async fn rocket(
    // #[shuttle_secrets::Secrets] secret_store: SecretStore,
    secret_store: SecretStore,
) -> shuttle_service::ShuttleRocket {
    // get secret defined in `Secrets.toml` file.
    let secret = if let Some(secret) = secret_store.get("MY_API_KEY") {
        secret
    } else {
        return Err(anyhow!("secret was not found").into());
    };

    let state = MyState { secret };
    let rocket = rocket::build()
        .mount("/", rocket::routes![secret])
        .manage(state);

    Ok(rocket)
}

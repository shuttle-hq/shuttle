# Shuttle Secrets

This plugin manages secrets on [Shuttle](https://www.shuttle.rs).

## Usage

Add `shuttle-secrets` to the dependencies for your service, and add a `Secrets.toml` to the root of your project
with the secrets you'd like to store. Make sure to add `Secrets*.toml` to a `.gitignore` to omit your secrets from version control.

Next, pass `#[shuttle_secrets::Secrets] secret_store: SecretStore` as an argument to your `shuttle_service::main` function.
`SecretStore::get` can now be called to retrieve your API keys and other secrets at runtime.

## Example

```rust,ignore
#[shuttle_runtime::main]
async fn rocket(
    #[shuttle_secrets::Secrets] secret_store: SecretStore,
) -> ShuttleRocket {
    // get secret defined in `Secrets.toml` file.
    let secret = if let Some(secret) = secret_store.get("MY_API_KEY") {
        secret
    } else {
        return Err(anyhow!("secret was not found").into());
    };

    let state = MyState { secret };
    let rocket = rocket::build().mount("/", routes![secret]).manage(state);

    Ok(rocket.into())
}
```

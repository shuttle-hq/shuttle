# Shuttle Secrets
This plugin manages secrets on [shuttle](https://www.shuttle.rs).

## Usage
Add `shuttle-secrets` to the dependencies for your service. Also add a dependency which will give you a `PgPool` like [shuttle-shared-db](https://github.com/shuttle-hq/shuttle/tree/main/resources/shared-db)

[`SecretStore::get_secret`] can now be called on any instance of this pool to retrieve stored secrets.

An example using the Rocket framework can be found on [GitHub](https://github.com/shuttle-hq/shuttle/tree/main/examples/rocket/postgres)


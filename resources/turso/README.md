# Shuttle Turso

This plugin allows services to connect to a Turso database. Turso is an edge-hosted distributed database based on libSQL.

## Usage

**IMPORTANT**: Currently Shuttle isn't able to provision a database for you (yet). This means you will have to create an account on their [website](https://turso.tech/) and follow the few steps required to create a database and create a token to access it.

Add `shuttle-turso` to the dependencies for your service.
This resource will be provided by adding the `shuttle_turso::Turso` attribute to your Shuttle `main` decorated function.

It returns a `libsql_client::Client`. When running in production, the token used to connect to your database will be read from `TURSO_DB_TOKEN` in your `Secrets.toml` file. This can be overridden with the `token_secret` parameter. When running locally it will instantiate a local SQLite database of the name of your service instead of connecting to your edge database.

If you want to connect to a remote database when running locally, you can specify the `local_addr` parameter. In that case, the token will be read from your `Secrets.dev.toml` file.

### Example

```rust
use libsql_client::client::Client;

#[shuttle_runtime::main]
async fn app(#[shuttle_turso::Turso(addr="libsql://advanced-lightspeed.turso.io")] client: Client) -> __ { }
```

### Parameters

| Parameter    | Type        | Default          | Description                                                                                                                   |
| ------------ | ----------- | ---------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| addr         | str         | `""`             | URL of the database to connect to. If `libsql://` is missing at the beginning, it will be automatically added.                |
| token_secret | str         | `TURSO_DB_TOKEN` | The name of the secret to read to get token created with the CLI to connect to the database.                                  |
| local_addr   | Option<str> | `None`           | The URL to use when running your service locally. If not provided, this will default to a local file name `<service name>.db` |

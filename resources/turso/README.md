# Shuttle Turso

This plugin allows services to connect to a Turso database. Turso is an edge-hosted distributed database based on libSQL, a SQLite fork.

## Usage

**IMPORTANT**: Currently Shuttle isn't able to provision a database for you (yet). This means you will have to create an account on their [website](https://turso.tech/) and follow the few steps required to create a database and create a token to access it.

Add `shuttle-turso` to the dependencies for your service by running `cargo add shuttle-turso`.
This resource will be provided by adding the `shuttle_turso::Turso` attribute to your Shuttle `main` decorated function.

It returns a `libsql::Connection`. When running locally it will instantiate a local SQLite database of the name of your service instead of connecting to your edge database.

If you want to connect to a remote database when running locally, you can specify the `local_addr` parameter. In that case, the token will be read from your `Secrets.dev.toml` file.

### Example

In the case of an Axum server, your main function will look like this:

```rust
use libsql::Connection;
use shuttle_axum::ShuttleAxum;

#[shuttle_runtime::main]
async fn app(
    #[shuttle_turso::Turso(
        addr="libsql://my-turso-db-name.turso.io",
        token="{secrets.DB_TURSO_TOKEN}"
    )] client: Connection,
) -> ShuttleAxum {}
```

### Parameters

| Parameter  | Type          | Default | Description |
| ---------- | ------------- | ------- | ----------- |
| addr       | `str`         | `""`    | URL of the database to connect to. Should begin with either `libsql://` or `https://`. |
| token      | `str`         | `""`    | The value of the token to authenticate against the Turso database. You can use string interpolation to read a secret from your `Secret.toml` file. |
| local_addr | `Option<str>` | `None`  | The URL to use when running your service locally. If not provided, this will default to a local file named `<service name>.db` |

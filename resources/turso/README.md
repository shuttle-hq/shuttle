# Shuttle Turso

This plugin allows services to connect to a Turso database. Turso is an edge-hosted distributed database based on libSQL.

## Usage

**IMPORTANT**: Currently Shuttle isn't able to provision a database for you (yet). This means you will have to create an account on their [website](https://turso.tech/) and follow the few steps required to create a database and create a token to access it.

Add `shuttle-turso` to the dependencies for your service.
This resource will be provided by adding the `shuttle_turso::Turso` attribute to your Shuttle `main` decorated function.

It returns a `libsql_client::Client`. When running locally it will instantiate a local SQLite database of the name of your service instead of connecting to your edge database.

### Example

```rust
#[shuttle_runtime::main]
async fn app(#[shuttle_turso::Turso(addr="advanced-lightspeed.turso.io", auth_token=Some("token")] client: Client) -> __ { }
```

### Parameters

| Parameter  | Type        | Default | Description                                                                                                                   |
| ---------- | ----------- | ------- | ----------------------------------------------------------------------------------------------------------------------------- |
| addr       | str         | `""`    | URL of the database to connect to, without the `libsql://` part at the beginning.                                             |
| token      | str         | `""`    | The auth token created with the CLI to connect to the database.                                                               |
| local_addr | Option<str> | `None`  | The URL to use when running your service locally. If not provided, this will default to a local file name `<service name>.db` |

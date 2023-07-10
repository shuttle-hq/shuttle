# Shuttle Sqlite

This plugin provides access to a SQLite database.

# Usage
Add `shuttle-sqlite` to the dependencies for your service and use the resource by adding the `shuttle_sqlite::ShuttleSqlite` 
attribute to main.

It returns a [`sqlx::SqlitePool`](https://docs.rs/sqlx/latest/sqlx/type.SqlitePool.html).

```rust
#[shuttle_runtime::main]
async fn axum(
    #[shuttle_sqlite::ShuttleSqlite] pool: shuttle_sqlite::SqlitePool,
) -> shuttle_axum::ShuttleAxum { /* ... */ }
```
Note that using `ShuttleAxum` is just an example, the resource can be used with any framework.

# Configuration
The database can be configured using [`ShuttleSqliteConnOpts`](https://docs.rs/shuttle-sqlite/latest/shuttle_sqlite/struct.ShuttleSqliteConnOpts.html) 
which mirrors sqlx's [`SqliteConnectOptions`](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html) 
for the options it exposes.

```rust
#[shuttle_runtime::main]
async fn axum(
    #[shuttle_sqlite::ShuttleSqlite(opts = ShuttleSqliteConnOpts::new().filename("custom.sqlite"))] pool: shuttle_sqlite::SqlitePool,
) -> shuttle_axum::ShuttleAxum { /* ... */ }
```
An example using this resource can be found in our [examples repo](https://github.com/shuttle-hq/shuttle-examples/tree/main/sqlite).
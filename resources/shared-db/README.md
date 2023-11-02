# Shuttle Shared Databases

This plugin manages databases that are shared with other services on [Shuttle](https://www.shuttle.rs).
Your database will be in a cluster shared with other users, but it will not be accessible by other users.

## Usage

Add `shuttle-shared-db` to the dependencies for your service. Every type of shareable database is behind the following feature flag and attribute path (`*-rustls` uses rustls for TLS, the default uses native-tls).

| Engine   | Feature flags                  | Attribute path              |
|----------|--------------------------------|-----------------------------|
| Postgres | `postgres` / `postgres-rustls` | shuttle_shared_db::Postgres |
| MongoDB  | `mongodb`                      | shuttle_shared_db::MongoDb  |

An example using the Rocket framework can be found on [GitHub](https://github.com/shuttle-hq/shuttle-examples/tree/main/rocket/postgres)

### Postgres

This resource has the following options

| Option    | Type | Description                                                                                                    |
|-----------|------|----------------------------------------------------------------------------------------------------------------|
| local_uri | &str | Don't spin a local docker instance of Postgres, but rather connect to this URI instead for `cargo shuttle run` |

### MongoDB

This resource has the following options

| Option    | Type | Description                                                                                                   |
|-----------|------|---------------------------------------------------------------------------------------------------------------|
| local_uri | &str | Don't spin a local docker instance of MongoDB, but rather connect to this URI instead for `cargo shuttle run` |

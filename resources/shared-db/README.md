# Shuttle Shared Databases
This plugin manages databases that are shared with other services on [shuttle](https://www.shuttle.rs).

## Usage
Add `shuttle-shared-db` to the dependencies for your service. Every type of shareable database is behind the following feature flag and attribute path

| Engine   | Feature flag | Attribute path              |
|----------|--------------|-----------------------------|
| Postgres | postgres     | shuttle_shared_db::Postgres |
| MongoDB  | mongodb      | shuttle_shared_db::MongoDb  |

An example using the Rocket framework can be found on [GitHub](https://github.com/shuttle-hq/examples/tree/main/rocket/postgres)

### Postgres
This resource has the following options

| Option    | Type | Description                                                                                                    |
|-----------|------|----------------------------------------------------------------------------------------------------------------|
| local_uri | &str | Don't spin a local docker instance of postgres, but rather connect to this URI instead for `cargo shuttle run` |


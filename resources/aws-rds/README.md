# Shuttle AWS RDS
This plugin provisions databases on AWS RDS using [shuttle](https://www.shuttle.rs). The following three engines are supported:
- Postgres
- MySql
- MariaDB

## Usage
Add `shuttle-aws-rds` to the dependencies for your service. Every engine is behind the following feature flags and attribute paths:

| Engine   | Feature flag | Attribute path            |
|----------|--------------|---------------------------|
| Postgres | postgres     | shuttle_aws_rds::Postgres |
| MySql    | mysql        | shuttle_aws_rds::MySql    |
| MariaDB  | mariadb      | shuttle_aws_rds::MariaDB  |

An example using the Tide framework can be found on [GitHub](https://github.com/shuttle-hq/examples/tree/main/tide/postgres)


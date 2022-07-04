pub enum Type {
    AwsRds(AwsRdsEngine),
    Shared,
}

pub enum AwsRdsEngine {
    Postgres,
    MySql,
    MariaDB,
}

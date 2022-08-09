pub enum Type {
    AwsRds(AwsRdsEngine),
    Shared(SharedEngine),
}

pub enum AwsRdsEngine {
    Postgres,
    MySql,
    MariaDB,
}

pub enum SharedEngine {
    Postgres,
    MongoDb,
}

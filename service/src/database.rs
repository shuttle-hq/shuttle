use std::fmt::Display;

pub enum Type {
    AwsRds(AwsRdsEngine),
    Shared,
}

pub enum AwsRdsEngine {
    Postgres,
    MySql,
    MariaDB,
}

impl Display for AwsRdsEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MariaDB => write!(f, "mariadb"),
            Self::MySql => write!(f, "mysql"),
            Self::Postgres => write!(f, "postgres"),
        }
    }
}

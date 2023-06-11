#[derive(Clone, Debug, Eq, PartialEq, sqlx::FromRow)]
pub struct User {
    pub api_key: String,
    pub gh_username: String,
}

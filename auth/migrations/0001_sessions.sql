CREATE TABLE IF NOT EXISTS sessions (
  session_token BYTEA PRIMARY KEY,
  account_name TEXT REFERENCES users(account_name) ON DELETE CASCADE,
  expiration TIMESTAMP
);

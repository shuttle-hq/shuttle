CREATE TABLE IF NOT EXISTS users (
  user_name TEXT PRIMARY KEY,
  public_key TEXT UNIQUE
);

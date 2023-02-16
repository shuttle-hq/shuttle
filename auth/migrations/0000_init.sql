CREATE TABLE IF NOT EXISTS users (
  account_name TEXT PRIMARY KEY,
  key TEXT UNIQUE,
  super_user BOOLEAN DEFAULT FALSE,
  account_tier TEXT DEFAULT "basic" NOT NULL
);

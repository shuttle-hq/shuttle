CREATE TABLE IF NOT EXISTS users (
  account_name TEXT PRIMARY KEY,
  key TEXT UNIQUE,
  account_tier TEXT DEFAULT "basic" NOT NULL
);

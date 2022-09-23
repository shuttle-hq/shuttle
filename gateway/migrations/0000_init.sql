CREATE TABLE IF NOT EXISTS projects (
  project_name TEXT PRIMARY KEY,
  account_name TEXT NOT NULL,
  initial_key TEXT NOT NULL,
  project_state JSON NOT NULL
);

CREATE TABLE IF NOT EXISTS accounts (
  account_name TEXT PRIMARY KEY,
  key TEXT UNIQUE,
  super_user BOOLEAN DEFAULT FALSE
);

CREATE TABLE IF NOT EXISTS projects (
  project_name TEXT PRIMARY KEY,
  account_name TEXT NOT NULL,
  initial_key TEXT NOT NULL,
  project_state JSON NOT NULL
  created_at INTEGER
);
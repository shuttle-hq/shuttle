CREATE TABLE IF NOT EXISTS services (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  state_variant TEXT NOT NULL,
  state JSON NOT NULL
);

CREATE TABLE IF NOT EXISTS deployments (
  id TEXT PRIMARY KEY,
  service_id TEXT,
  state TEXT NOT NULL,
  address TEXT,
  last_update INTEGER,
  is_next BOOLEAN,
  git_commit_hash TEXT,
  git_commit_message TEXT,
  git_branch TEXT,
  git_dirty BOOLEAN,
  FOREIGN KEY (service_id) REFERENCES services (id)
);
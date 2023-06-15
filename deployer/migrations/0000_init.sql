CREATE TABLE IF NOT EXISTS services (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  state_variant TEXT NOT NULL,
  state JSON NOT NULL
);

CREATE TABLE IF NOT EXISTS deployments (
  id TEXT PRIMARY KEY,
  service_id TEXT NOT NULL,
  state TEXT NOT NULL,
  last_update INTEGER NOT NULL,
  is_next BOOLEAN NOT NULL,
  git_commit_hash TEXT,
  git_commit_message TEXT,
  git_branch TEXT,
  git_dirty BOOLEAN,
  FOREIGN KEY (service_id) REFERENCES services (id)
);
CREATE TABLE IF NOT EXISTS deployments (
  id TEXT PRIMARY KEY,
  FOREIGN KEY(service_id) REFERENCES services(id),
  state TEXT NOT NULL,
  address TEXT,
  is_next BOOLEAN,
  git_commit_hash TEXT,
  git_commit_message TEXT,
  git_branch TEXT,
  git_dirty BOOLEAN,
);

CREATE TABLE IF NOT EXISTS services (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  state_variant TEXT NOT NULL,
  state JSON NOT NULL,
);
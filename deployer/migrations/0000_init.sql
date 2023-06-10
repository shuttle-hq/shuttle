CREATE TABLE IF NOT EXISTS deployments (
  deployment_id TEXT PRIMARY KEY,
  service_id TEXT NOT NULL,
  address TEXT NOT NULL,
  state_variant TEXT NOT NULL,
  state JSON NOT NULL,
  git_commit_hash TEXT,
  git_commit_message TEXT,
  git_branch TEXT,
  git_dirty BOOLEAN,
);

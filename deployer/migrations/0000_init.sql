CREATE TABLE IF NOT EXISTS deployments (
  deployment_id TEXT PRIMARY KEY,
  service_id TEXT NOT NULL,
  address TEXT NOT NULL,
  state TEXT NOT NULL,
  raw_state JSON NOT NULL,
);

CREATE TABLE IF NOT EXISTS projects (
  project_id TEXT PRIMARY KEY,
  service_id TEXT NOT NULL,
  state TEXT NOT NULL,
  raw_state JSON NOT NULL,
);

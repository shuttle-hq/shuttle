CREATE TABLE IF NOT EXISTS services (
  id TEXT PRIMARY KEY,                 -- The service id in ulid format
  name TEXT NOT NULL,                  -- The service name
  state_variant TEXT NOT NULL,         -- The service state variant
  state JSON NOT NULL                  -- The serialized docker container inspect state
  project_id TEXT NOT NULL             -- The project id that owns the service
  last_update INTEGER NOT NULL         -- The timestamp of the last service update
);

CREATE TABLE IF NOT EXISTS deployments (
  id TEXT PRIMARY KEY,                                -- The deployment id
  service_id TEXT NOT NULL,                           -- The associated service id
  last_update INTEGER NOT NULL,                       -- Last time this update was modified
  is_next BOOLEAN NOT NULL,                           -- Whether it's a next deployment or not
  git_commit_hash TEXT,                               -- Associated git commit hash the deployment points to
  git_commit_message TEXT,                            -- Associated git commit message of the last commit where the deployment points to
  git_branch TEXT,                                    -- Associated git branch used for the deployment
  git_dirty BOOLEAN,                                  -- Whether the git branch was dirty before the deployment started
  FOREIGN KEY (service_id) REFERENCES services (id)
);
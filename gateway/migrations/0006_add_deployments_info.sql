CREATE TABLE IF NOT EXISTS services (
    id TEXT PRIMARY KEY,    -- Identifier of the service.
    project_id ULID,        -- The project the service is bound to
    name TEXT,              -- Name of the service.
    FOREIGN KEY(project_id) REFERENCES projects(id)
);

CREATE TABLE IF NOT EXISTS deployments (
    id TEXT PRIMARY KEY, -- Identifier of the deployment.
    service_id TEXT,     -- Identifier of the service this deployment belongs to.
    state TEXT,          -- Enum indicating the current state of the deployment.
    last_update INTEGER, -- Unix epoch of the last status update
    address TEXT,        -- Address a running deployment is active on
    is_next BOOLEAN DEFAULT 0 NOT NULL, -- Whether the deployment is based on a shuttle-next runtime
    git_commit_id TEXT,  -- Commit id of the underlying git repo of the deployed project
    git_commit_msg TEXT, -- Commit message of the underlying git repo of the deployed project
    git_branch TEXT,     -- Git branch of the underlying git repo of the deployed project
    git_dirty BOOLEAN,   -- Whether the deployed project had uncommited git changes
    FOREIGN KEY(service_id) REFERENCES services(id)
);
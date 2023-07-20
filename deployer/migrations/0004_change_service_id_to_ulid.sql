-- Copy current service table. Keep the old table around because
-- the rest of the tables are still having an FK on service_id.
CREATE TABLE IF NOT EXISTS services_copy (
    id TEXT PRIMARY KEY, -- Identifier of the service.
    name TEXT UNIQUE     -- Name of the service.
);

INSERT INTO services_copy (id, name)
SELECT
  uuid_to_ulid(services.id),
  services.name
FROM services;

-- Copy current deployments table without the FK service_id constraint.
CREATE TABLE IF NOT EXISTS deployments_copy (
    id TEXT PRIMARY KEY, -- Identifier of the deployment.
    service_id TEXT,     -- Identifier of the service this deployment belongs to.
    state TEXT,          -- Enum indicating the current state of the deployment.
    last_update INTEGER, -- Unix epoch of the last status update
    address TEXT,        -- Address a running deployment is active on
    is_next BOOLEAN,     -- Whether the deployment is for a shuttle-next runtime
    git_commit_id TEXT,  -- Deployment git commit id
    git_commit_msg TEXT, -- Deployment last git commit msg
    git_branch TEXT,     -- Deployment git branch
    git_dirty BOOLEAN    -- Deployment git state is dirty
);

INSERT INTO deployments_copy (id, service_id, state, last_update, address, is_next)
SELECT
  deployments.id,
  uuid_to_ulid(deployments.service_id),
  deployments.state,
  deployments.last_update,
  deployments.address,
  deployments.is_next,
  deployments.git_commit_id,
  deployments.git_commit_msg,
  deployments.git_branch,
  deployments.git_dirty
FROM deployments;

-- Copy current resource table without the FK service_id constraint.
CREATE TABLE IF NOT EXISTS resources_copy (
    service_id TEXT,   -- Identifier of the service this resource belongs to.
    type TEXT,         -- Type of resource this is.
    data TEXT,         -- Data about this resource.
    config TEXT,       -- Resource configuration.
    PRIMARY KEY (service_id, type),
);
INSERT INTO resources_copy (service_id, type, data, config)
SELECT
  uuid_to_ulid(resources.service_id),
  resources.type,
  resources.data,
  resources.config,
FROM resources;

-- Copy current secrets table without the FK service_id constraint.
CREATE TABLE IF NOT EXISTS secrets_copy (
    service_id TEXT,      -- Identifier of the service this secret belongs to.
    key TEXT,             -- Key / name of this secret.
    value TEXT,           -- The actual secret.
    last_update INTEGER,  -- Unix epoch of the last secret update
    PRIMARY KEY (service_id, key),
);
INSERT INTO secrets_copy (service_id, key, value, last_update)
SELECT
  uuid_to_ulid(secrets.service_id),
  secrets.key,
  secrets.value,
  secrets.last_update
FROM secrets;

-- Recreate the deployments table with an FK constraint on the service_id.
DROP TABLE deployments;
CREATE TABLE IF NOT EXISTS deployments (
    id TEXT PRIMARY KEY, -- Identifier of the deployment.
    service_id TEXT,     -- Identifier of the service this deployment belongs to.
    state TEXT,          -- Enum indicating the current state of the deployment.
    last_update INTEGER, -- Unix epoch of the last status update
    address TEXT         -- Address a running deployment is active on
    is_next BOOLEAN,     -- Whether the deployment is for a shuttle-next runtime
    git_commit_id TEXT,  -- Deployment git commit id
    git_commit_msg TEXT, -- Deployment last git commit msg
    git_branch TEXT,     -- Deployment git branch
    git_dirty BOOLEAN    -- Deployment git state is dirty
    FOREIGN KEY(service_id) REFERENCES services(id)
);
INSERT INTO deployments SELECT * FROM deployments_copy;
DROP TABLE deployments_copy;

-- Recreate the resources table with an FK constraint on the service_id.
DROP TABLE resources;
CREATE TABLE IF NOT EXISTS resources (
    service_id TEXT,   -- Identifier of the service this resource belongs to.
    type TEXT,         -- Type of resource this is.
    data TEXT,         -- Data about this resource.
    config TEXT,       -- Resource configuration.
    PRIMARY KEY (service_id, type),
    FOREIGN KEY(service_id) REFERENCES services(id)
);
INSERT INTO resources SELECT * FROM resources_copy;
DROP TABLE resources_copy;

-- Replace the old services table with the updated one.
DROP TABLE services;
ALTER TABLE services_copy RENAME TO services;

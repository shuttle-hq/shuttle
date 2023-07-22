-- Copy current service table. Keep the old table around because
-- the rest of the tables are still having an FK on service_id.
CREATE TABLE IF NOT EXISTS services_copy (
    id TEXT PRIMARY KEY,  -- Ulid identifier of the service.
    uuid TEXT,            -- Old Uuid identifier of the service.
    name TEXT UNIQUE      -- Name of the service.
);

INSERT INTO services_copy (id, uuid, name)
SELECT
  upper(ulid_with_datetime(strftime('%Y-%m-%d %H:%M:%f', datetime('2023-06-01')))) as id,
  services.id as uuid,
  services.name
FROM services;

-- Copy current deployments table without the FK service_id constraint.
CREATE TABLE IF NOT EXISTS deployments_copy (
    id TEXT PRIMARY KEY, -- Identifier of the deployment.
    service_id TEXT,     -- Identifier of the service this deployment belongs to.
    state TEXT,          -- Enum indicating the current state of the deployment.
    last_update INTEGER, -- Unix epoch of the last status update
    address TEXT,        -- Address a running deployment is active on
    is_next BOOLEAN DEFAULT 0 NOT NULL,     -- Whether the deployment is for a shuttle-next runtime
    git_commit_id TEXT,  -- Deployment git commit id
    git_commit_msg TEXT, -- Deployment last git commit msg
    git_branch TEXT,     -- Deployment git branch
    git_dirty BOOLEAN    -- Deployment git state is dirty
);

INSERT INTO deployments_copy (id, service_id, state, last_update, address, is_next, git_commit_id, git_commit_msg, git_branch, git_dirty)
SELECT
  deployments.id,
  services_copy.id as service_id, -- Copy the generated ulid from the related services_copy row.
  deployments.state,
  deployments.last_update,
  deployments.address,
  deployments.is_next,
  deployments.git_commit_id,
  deployments.git_commit_msg,
  deployments.git_branch,
  deployments.git_dirty
FROM deployments
JOIN services_copy ON services_copy.uuid = deployments.service_id;

-- Copy current resource table without the FK service_id constraint.
CREATE TABLE IF NOT EXISTS resources_copy (
    service_id TEXT,   -- Identifier of the service this resource belongs to.
    type TEXT,         -- Type of resource this is.
    data TEXT,         -- Data about this resource.
    config TEXT,       -- Resource configuration.
    PRIMARY KEY (service_id, type)
);
INSERT INTO resources_copy (service_id, type, data, config)
SELECT
  services_copy.id as service_id,
  resources.type,
  resources.data,
  resources.config
FROM resources
JOIN services_copy ON services_copy.uuid = resources.service_id;

-- Copy current secrets table without the FK service_id constraint.
CREATE TABLE IF NOT EXISTS secrets_copy (
    service_id TEXT,      -- Identifier of the service this secret belongs to.
    key TEXT,             -- Key / name of this secret.
    value TEXT,           -- The actual secret.
    last_update INTEGER,  -- Unix epoch of the last secret update
    PRIMARY KEY (service_id, key)
);
INSERT INTO secrets_copy (service_id, key, value, last_update)
SELECT
  services_copy.id as service_id, -- Copy the generated ulid from the related services_copy row.
  secrets.key,
  secrets.value,
  secrets.last_update
FROM secrets
JOIN services_copy ON services_copy.uuid = secrets.service_id;

-- We can safely drop the uuid column now, since we don't need it anymore.
ALTER TABLE services_copy DROP COLUMN uuid;

-- Make a logs_copy first without the deployments FK.
CREATE TABLE IF NOT EXISTS logs_copy (
    id TEXT,           -- The deployment that this log line pertains to.
    timestamp INTEGER, -- Unix epoch timestamp.
    state TEXT,        -- The state of the deployment at the time at which the log text was produced.
    level TEXT,        -- The log level
    file TEXT,         -- The file log took place in
    line INTEGER,      -- The line log took place on
    target TEXT,       -- The module log took place in
    fields TEXT,       -- Log fields object.
    PRIMARY KEY (id, timestamp)
);
INSERT INTO logs_copy (id, timestamp, state, level, file, line, target, fields)
SELECT
  logs.id,
  logs.timestamp,
  logs.state,
  logs.level,
  logs.file,
  logs.line,
  logs.target,
  logs.fields
FROM logs;

-- Recreate the deployments table with an FK constraint on the service_id.
DROP TABLE logs;
DROP TABLE deployments;
CREATE TABLE IF NOT EXISTS deployments (
    id TEXT PRIMARY KEY, -- Identifier of the deployment.
    service_id TEXT,     -- Identifier of the service this deployment belongs to.
    state TEXT,          -- Enum indicating the current state of the deployment.
    last_update INTEGER, -- Unix epoch of the last status update
    address TEXT,        -- Address a running deployment is active on
    is_next BOOLEAN DEFAULT 0 NOT NULL,     -- Whether the deployment is for a shuttle-next runtime
    git_commit_id TEXT,  -- Deployment git commit id
    git_commit_msg TEXT, -- Deployment last git commit msg
    git_branch TEXT,     -- Deployment git branch
    git_dirty BOOLEAN,   -- Deployment git state is dirty
    FOREIGN KEY(service_id) REFERENCES services_copy(id)
);
INSERT INTO deployments SELECT * FROM deployments_copy;
DROP TABLE deployments_copy;

-- Recreate logs table with FK on deployments ID.
CREATE TABLE IF NOT EXISTS logs (
    id TEXT,           -- The deployment that this log line pertains to.
    timestamp INTEGER, -- Unix epoch timestamp.
    state TEXT,        -- The state of the deployment at the time at which the log text was produced.
    level TEXT,        -- The log level
    file TEXT,         -- The file log took place in
    line INTEGER,      -- The line log took place on
    target TEXT,       -- The module log took place in
    fields TEXT,       -- Log fields object.
    PRIMARY KEY (id, timestamp),
    FOREIGN KEY(id) REFERENCES deployments(id)
);
INSERT INTO logs SELECT * FROM logs_copy;
DROP TABLE logs_copy;

-- Recreate the resources table with an FK constraint on the service_id.
DROP TABLE resources;
CREATE TABLE IF NOT EXISTS resources (
    service_id TEXT,   -- Identifier of the service this resource belongs to.
    type TEXT,         -- Type of resource this is.
    data TEXT,         -- Data about this resource.
    config TEXT,       -- Resource configuration.
    PRIMARY KEY (service_id, type),
    FOREIGN KEY(service_id) REFERENCES services_copy(id)
);
INSERT INTO resources SELECT * FROM resources_copy;
DROP TABLE resources_copy;

-- Recreate the secrets table with an FK constraint on the service_id.
DROP TABLE secrets;
CREATE TABLE IF NOT EXISTS secrets (
    service_id TEXT,      -- Identifier of the service this secret belongs to.
    key TEXT,             -- Key / name of this secret.
    value TEXT,           -- The actual secret.
    last_update INTEGER,  -- Unix epoch of the last secret update
    PRIMARY KEY (service_id, key),
    FOREIGN KEY(service_id) REFERENCES services_copy(id)
);
INSERT INTO secrets SELECT * FROM secrets_copy;
DROP TABLE secrets_copy;

-- Replace the old services table with the updated one.
DROP TABLE services;
ALTER TABLE services_copy RENAME TO services;

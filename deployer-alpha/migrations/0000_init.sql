CREATE TABLE IF NOT EXISTS services (
    id TEXT PRIMARY KEY, -- Identifier of the service.
    name TEXT UNIQUE     -- Name of the service.
);

CREATE TABLE IF NOT EXISTS deployments (
    id TEXT PRIMARY KEY, -- Identifier of the deployment.
    service_id TEXT,     -- Identifier of the service this deployment belongs to.
    state TEXT,          -- Enum indicating the current state of the deployment.
    last_update INTEGER, -- Unix epoch of the last status update
    address TEXT,        -- Address a running deployment is active on
    FOREIGN KEY(service_id) REFERENCES services(id)
);

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

CREATE TABLE IF NOT EXISTS resources (
    service_id TEXT,   -- Identifier of the service this resource belongs to.
    type TEXT,         -- Type of resource this is.
    data TEXT,         -- Data about this resource.
    PRIMARY KEY (service_id, type),
    FOREIGN KEY(service_id) REFERENCES services(id)
);

CREATE TABLE IF NOT EXISTS secrets (
    service_id TEXT,      -- Identifier of the service this secret belongs to.
    key TEXT,             -- Key / name of this secret.
    value TEXT,           -- The actual secret.
    last_update INTEGER,  -- Unix epoch of the last secret update
    PRIMARY KEY (service_id, key),
    FOREIGN KEY(service_id) REFERENCES services(id)
);

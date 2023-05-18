CREATE TABLE IF NOT EXISTS resources (
    project_id TEXT,                                  -- Identifier of the project this resource belongs to.
    service_id TEXT,                                  -- Identifier of the service this resource belongs to.
    type TEXT,                                        -- Type of resource this is.
    data TEXT,                                        -- Data about this resource.
    config TEXT,                                      -- The config to create the object for this resource.
    is_active boolean,                                -- Flag telling whether the resource is being actively used.
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,   -- Time this resource was created.
    last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP, -- Time this resource was last updated.
    PRIMARY KEY (project_id, service_id, type)
);

CREATE INDEX IF NOT EXISTS project_id_idx ON resources(project_id);
CREATE INDEX IF NOT EXISTS service_id_idx ON resources(service_id);

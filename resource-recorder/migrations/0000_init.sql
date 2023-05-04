CREATE TABLE IF NOT EXISTS resources (
    id TEXT,           -- Identifier of this resource.
    service_id TEXT,   -- Identifier of the service this resource belongs to.
    type TEXT,         -- Type of resource this is.
    data TEXT,         -- Data about this resource.
    config TEXT,       -- The config to create the object for this resource.
    PRIMARY KEY (service_id, type)
);

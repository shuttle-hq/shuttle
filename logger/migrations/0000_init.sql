CREATE TABLE IF NOT EXISTS logs (
    deployment_id TEXT,        -- The deployment that this log line pertains to.
    shuttle_service_name TEXT, -- The shuttle service which created this log.
    tx_timestamp TIMESTAMPTZ,  -- Unix epoch timestamp.
    data BYTEA                 -- Log line.
);

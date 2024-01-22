-- Fix lowercase ulids that were inserted before https://github.com/shuttle-hq/shuttle/commit/96105b4c8fd14642d4c70fcfdb231e1e5b8a0d65

CREATE TABLE projects_copy (
  project_id ULID PRIMARY KEY,
  project_name TEXT UNIQUE,
  account_name TEXT NOT NULL,
  initial_key TEXT NOT NULL,
  project_state JSON NOT NULL
);
INSERT INTO projects_copy SELECT * FROM projects;
UPDATE projects_copy SET project_id = upper(project_id); -- uppercase the old ulids

CREATE TABLE custom_domains_copy (
  fqdn TEXT PRIMARY KEY,
  project_id ULID NOT NULL, -- First create the table without the FK constraint on project_id.
  certificate TEXT NOT NULL,
  private_key TEXT NOT NULL
);
INSERT INTO custom_domains_copy SELECT * FROM custom_domains;
UPDATE custom_domains_copy SET project_id = upper(project_id); -- uppercase the old ulids

-- Drop this first, it has an FK to projects.
DROP TABLE custom_domains;

-- Replace the old projects table with the updated one.
DROP TABLE projects;
ALTER TABLE projects_copy RENAME TO projects;

-- Recreate the custom_domains table with an FK constraint on the project_id.
CREATE TABLE custom_domains (
  fqdn TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects (project_id),
  certificate TEXT NOT NULL,
  private_key TEXT NOT NULL
);

INSERT INTO custom_domains SELECT * FROM custom_domains_copy;
DROP TABLE custom_domains_copy;

UPDATE projects SET created_at = datetime('2023-06-01') WHERE created_at IS NULL;

-- We need to create a copy of the table to alter the primary key to the new project_id column.
CREATE TABLE IF NOT EXISTS projects_copy (
  project_id ULID PRIMARY KEY,
  project_name TEXT UNIQUE,
  account_name TEXT NOT NULL,
  initial_key TEXT NOT NULL,
  project_state JSON NOT NULL
);

-- We use the https://github.com/asg017/sqlite-ulid extension to generate the ulid for new table.
INSERT INTO projects_copy (project_id, project_name, account_name, initial_key, project_state)
SELECT 
  ulid_with_datetime(strftime('%Y-%m-%d %H:%M:%f', projects.created_at)),
  projects.project_name,
  projects.account_name,
  projects.initial_key,
  projects.project_state
FROM projects;

-- We need to create a copy of the table to be able to alter the foreign key, it was previously
-- on project_name but will now be on the new project_id (ULID) column.
CREATE TABLE IF NOT EXISTS custom_domains_copy (
  fqdn TEXT PRIMARY KEY,
  project_id ULID NOT NULL, -- First create the table without the FK constraint on project_id.
  certificate TEXT NOT NULL,
  private_key TEXT NOT NULL
);

INSERT INTO custom_domains_copy (fqdn, project_id, certificate, private_key)
SELECT 
  custom_domains.fqdn,
  projects_copy.project_id, -- Copy the generated ulid from the related projects_copy row.
  custom_domains.certificate,
  custom_domains.private_key
FROM custom_domains
JOIN projects_copy ON projects_copy.project_name = custom_domains.project_name;

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

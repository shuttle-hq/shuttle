CREATE TABLE IF NOT EXISTS custom_domains (
  fqdn TEXT PRIMARY KEY,
  project_name TEXT NOT NULL REFERENCES projects (project_name),
  state JSON NOT NULL
);

CREATE INDEX IF NOT EXISTS custom_domains_fqdn_project_idx ON custom_domains (fqdn, project_name);

CREATE TABLE IF NOT EXISTS custom_domains (
  fqdn TEXT PRIMARY KEY,
  project_name TEXT NOT NULL REFERENCES projects (project_name),
  certificate TEXT NOT NULL,
  private_key TEXT NOT NULL
);

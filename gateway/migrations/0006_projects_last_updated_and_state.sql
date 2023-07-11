/*
Recreate the projects table with the following changes:
- Change project_state to TEXT rather than JSON, we only want the state variant now.
- Drop initial_key column.
- Add the last_updated column.
- Add the address column, this will hold the IP traffic to this project should be proxied to.
*/
CREATE TABLE projects_copy (
  project_id TEXT PRIMARY KEY,
  project_name TEXT UNIQUE,
  account_name TEXT NOT NULL,
  project_state TEXT NOT NULL,
  address TEXT,  -- The IP of the container to proxy traffic to. TODO: should this be NOT NULL?
  last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  created_at INTEGER
);

INSERT INTO projects_copy (project_id, project_name, account_name, project_state, created_at)
SELECT 
  projects.project_id,
  projects.project_name,
  projects.account_name,
  json_each.key,          -- Extract the state variant from the old json (it was the root json key).
  projects.created_at
FROM projects, json_each(projects.project_state);

DROP TABLE projects;

ALTER TABLE projects_copy RENAME TO projects;

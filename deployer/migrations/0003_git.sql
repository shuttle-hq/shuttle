ALTER TABLE deployments
ADD COLUMN git_commit_id TEXT;

ALTER TABLE deployments
ADD COLUMN git_commit_msg TEXT;

ALTER TABLE deployments
ADD COLUMN git_branch TEXT;

ALTER TABLE deployments
ADD COLUMN git_dirty BOOLEAN;
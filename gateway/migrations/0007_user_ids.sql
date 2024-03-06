-- Add new user_id column for projects
ALTER TABLE projects
ADD COLUMN user_id TEXT;

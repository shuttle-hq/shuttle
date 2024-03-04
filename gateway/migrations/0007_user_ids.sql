-- Add new user_id column for projects
ALTER TABLE users
ADD COLUMN user_id UUID;

-- Add new user_id column for users
ALTER TABLE users
ADD COLUMN user_id TEXT UNIQUE;
-- All NULL values are filled in at runtime

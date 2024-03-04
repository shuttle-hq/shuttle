-- Add new user_id column for users
ALTER TABLE users
ADD COLUMN user_id UUID DEFAULT gen_random_uuid() UNIQUE;

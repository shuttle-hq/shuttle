CREATE TABLE IF NOT EXISTS subscriptions (
  subscription_id TEXT PRIMARY KEY,
  account_name TEXT NOT NULL,
  type TEXT NOT NULL,
  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, 
  UNIQUE (account_name, type),
  FOREIGN KEY (account_name) REFERENCES users(account_name)
);

-- Create a trigger to automatically update updated_at
CREATE TRIGGER sync_users_updated_at
BEFORE UPDATE
ON subscriptions
FOR EACH ROW
EXECUTE PROCEDURE sync_updated_at();

-- Insert existing subscriptions into the new subscriptions table
INSERT INTO subscriptions (subscription_id, account_name, type)
SELECT subscription_id, account_name, 'pro'
FROM users
WHERE subscription_id IS NOT NULL;

-- Drop the subscription_id column from the users table
ALTER TABLE users
DROP COLUMN subscription_id;

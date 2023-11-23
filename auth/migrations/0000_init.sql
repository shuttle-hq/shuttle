CREATE TYPE tier AS ENUM ('basic', 'pendingpaymentpro', 'pro', 'team', 'admin', 'deployer', 'cancelledpro');
CREATE TABLE IF NOT EXISTS users (
  account_name TEXT PRIMARY KEY,
  key TEXT UNIQUE,
  account_tier tier DEFAULT 'basic' NOT NULL,
  subscription_id TEXT
);
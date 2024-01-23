-- The order of the variants gives the ordering for the type, so it would be best to keep them in lexicographical order.
CREATE TYPE tier AS ENUM ('admin', 'basic', 'cancelledpro', 'deployer', 'pendingpaymentpro', 'pro', 'team');
ALTER TABLE users ALTER COLUMN account_tier DROP DEFAULT;
ALTER TABLE users ALTER COLUMN account_tier TYPE tier USING account_tier::tier;
ALTER TABLE users ALTER COLUMN account_tier SET DEFAULT 'basic';
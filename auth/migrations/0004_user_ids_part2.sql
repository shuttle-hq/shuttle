-------- Fixes for inconsistencies caused by migration from sqlite
ALTER TABLE users
ALTER COLUMN account_tier SET DEFAULT 'basic',
ALTER COLUMN account_tier SET NOT NULL;

DO $$
BEGIN
    -- staging
    IF EXISTS (SELECT 1 FROM information_schema.constraint_column_usage WHERE table_name = 'users' AND constraint_name = 'idx_16440_sqlite_autoindex_users_1') THEN
        EXECUTE 'ALTER TABLE users RENAME CONSTRAINT idx_16440_sqlite_autoindex_users_1 TO users_pkey';
    END IF;
    IF EXISTS (SELECT 1 FROM pg_indexes WHERE tablename = 'users' AND indexname = 'idx_16440_sqlite_autoindex_users_2') THEN
        EXECUTE 'ALTER TABLE users DROP CONSTRAINT idx_16440_sqlite_autoindex_users_2';
        EXECUTE 'ALTER TABLE users ADD CONSTRAINT users_key_key UNIQUE(key)';
    END IF;
    -- prod
    IF EXISTS (SELECT 1 FROM information_schema.constraint_column_usage WHERE table_name = 'users' AND constraint_name = 'idx_20519_sqlite_autoindex_users_1') THEN
        EXECUTE 'ALTER TABLE users RENAME CONSTRAINT idx_20519_sqlite_autoindex_users_1 TO users_pkey';
    END IF;
    IF EXISTS (SELECT 1 FROM pg_indexes WHERE tablename = 'users' AND indexname = 'idx_20519_sqlite_autoindex_users_2') THEN
        EXECUTE 'ALTER TABLE users DROP CONSTRAINT idx_20519_sqlite_autoindex_users_2';
        EXECUTE 'ALTER TABLE users ADD CONSTRAINT users_key_key UNIQUE(key)';
    END IF;
END$$;
--------


-- All rows should have user_ids at this point (added in the application logic before this migration was introduced)
ALTER TABLE users
ALTER COLUMN user_id SET NOT NULL;


-- Switch the foreign key(fk) on subscriptions and remove the old fk
ALTER TABLE subscriptions
ADD COLUMN user_id TEXT;

UPDATE subscriptions
SET user_id = users.user_id
FROM users
WHERE subscriptions.account_name = users.account_name;

ALTER TABLE subscriptions
DROP CONSTRAINT subscriptions_account_name_fkey,
ADD FOREIGN KEY (user_id) REFERENCES users (user_id),
ALTER COLUMN user_id SET NOT NULL,
DROP COLUMN account_name,
-- Add back the unique pair constraint
ADD CONSTRAINT user_id_type UNIQUE (user_id, type);


-- Switch the primary key on users
ALTER TABLE users
DROP CONSTRAINT users_pkey,
ADD PRIMARY KEY (user_id);

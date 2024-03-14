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

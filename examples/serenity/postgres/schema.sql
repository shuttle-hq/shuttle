DROP TABLE IF EXISTS todos;

CREATE TABLE todos (
  id serial PRIMARY KEY,
  user_id BIGINT NULL,
  note TEXT NOT NULL
);

DROP TABLE IF EXISTS todos;

CREATE TABLE todos (
  id serial PRIMARY KEY,
  note TEXT NOT NULL, -- this comma is invalid to test failures in state()
);

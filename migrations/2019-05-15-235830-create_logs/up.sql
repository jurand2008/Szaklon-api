CREATE TABLE logs (
  id INTEGER NOT NULL PRIMARY KEY,
  login TEXT NOT NULL,
  logging_time TIMESTAMP NOT NULL,
  logging_succession BOOLEAN NOT NULL
)

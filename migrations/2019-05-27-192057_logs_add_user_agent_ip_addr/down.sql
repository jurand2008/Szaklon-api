CREATE TEMPORARY TABLE logs_bk(id, login, logging_time, logging_succession);
INSERT INTO logs_bk SELECT id, login, logging_time, logging_succession FROM logs;
DROP TABLE logs;
CREATE TABLE logs (
  id INTEGER NOT NULL PRIMARY KEY,
  login TEXT NOT NULL,
  logging_time TIMESTAMP NOT NULL,
  logging_succession BOOLEAN NOT NULL
);
INSERT INTO logs SELECT id, login, logging_time, logging_succession FROM logs_bk;
DROP TABLE logs_bk;

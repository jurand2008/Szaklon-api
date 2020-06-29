CREATE TEMPORARY TABLE users_bk(id, login, hash, role);
INSERT INTO users_bk SELECT id, login, hash, role FROM users;
DROP TABLE users;
CREATE TABLE users (
    id INTEGER NOT NULL PRIMARY KEY,
    login TEXT NOT NULL,
    hash TEXT NOT NULL,
    role TEXT NOT NULL
);
INSERT INTO users SELECT id, login, hash, role FROM users_bk;
DROP TABLE users_bk;

CREATE TEMPORARY TABLE songs_bk(id, artist, title, genre, url);
INSERT INTO songs_bk SELECT id, artist, title, genre, url FROM songs;
DROP TABLE songs;
CREATE TABLE songs (
    id INTEGER NOT NULL PRIMARY KEY,
    artist TEXT NOT NULL,
    title TEXT NOT NULL,
    genre TEXT NOT NULL,
    url TEXT NOT NULL
);
INSERT INTO songs SELECT id, artist, title, genre, url FROM songs_bk;
DROP TABLE songs_bk;

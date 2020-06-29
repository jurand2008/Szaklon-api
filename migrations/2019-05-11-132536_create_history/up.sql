CREATE TABLE history (
    id INTEGER PRIMARY KEY NOT NULL,
    user_id INTEGER REFERENCES users(id),
    song_id INTEGER NOT NULL REFERENCES songs(id),
    matched_at TIMESTAMP NOT NULL
)

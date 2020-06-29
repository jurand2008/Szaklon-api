use actix::prelude::*;
use actix_web::{dev::Body, http::StatusCode, web::HttpResponse, ResponseError};
use diesel::prelude::*;
use diesel::sql_types::Integer;
use failure_derive::Fail;
use serde::Deserialize;

use crate::db::models::{NewHistory, Song};
use crate::db::schema::songs;
use crate::db::DbExecutor;
use crate::songs::{HistoryEntry, TopSong};

pub struct Recognize {
    pub song_ids: Vec<i32>,
    pub user_id: Option<i32>,
}

pub struct GetHistory {
    pub user_id: Option<i32>,
}

pub struct GetMostPopular {
    pub limit: u32,
}

/// Send empty vector to disable filtering.
#[derive(Deserialize)]
pub struct GetAllSongs {
    pub genres: Vec<String>,
    pub artists: Vec<String>,
    #[serde(default)]
    pub featured: Option<bool>,
}

pub struct EditSong {
    pub song: Song,
}

#[derive(Deserialize, Insertable)]
#[table_name = "songs"]
pub struct AddSong {
    pub artist: String,
    pub title: String,
    pub genre: String,
    pub url: String,
}

pub struct GetAllGenres;

pub struct GetAllArtists;

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "Song was not found")]
    NotFound,
    #[fail(display = "Database error: {}", _0)]
    DbError(#[cause] diesel::result::Error),
}

impl ResponseError for Error {
    fn error_response(&self) -> HttpResponse<Body> {
        match self {
            Error::DbError(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
            Error::NotFound => HttpResponse::new(StatusCode::NOT_FOUND),
        }
    }
}

impl From<diesel::result::Error> for Error {
    fn from(f: diesel::result::Error) -> Self {
        Error::DbError(f)
    }
}

impl Message for Recognize {
    type Result = Result<Vec<Song>, Error>;
}

impl Handler<Recognize> for DbExecutor {
    type Result = Result<Vec<Song>, Error>;

    fn handle(&mut self, msg: Recognize, _: &mut Self::Context) -> Self::Result {
        use super::schema::history::dsl::history;
        use super::schema::songs::dsl::songs;

        let mut sgs = Vec::new();
        for song_id in &msg.song_ids {
            sgs.push(songs.find(song_id).first(&self.0)?);
        }

        let history_entry = NewHistory {
            song_id: msg.song_ids[0],
            user_id: msg.user_id,
            matched_at: chrono::offset::Utc::now().naive_utc(),
        };

        diesel::insert_into(history)
            .values(&history_entry)
            .execute(&self.0)?;

        Ok(sgs)
    }
}

impl Message for GetHistory {
    type Result = Result<Vec<HistoryEntry>, Error>;
}

impl Handler<GetHistory> for DbExecutor {
    type Result = Result<Vec<HistoryEntry>, Error>;

    fn handle(&mut self, msg: GetHistory, _: &mut Self::Context) -> Self::Result {
        use super::schema::history::dsl::{history, matched_at, song_id, user_id};
        use super::schema::songs::dsl::{artist, genre, songs, title, url};

        let entries = if let Some(uid) = msg.user_id {
            history
                .filter(user_id.eq(uid))
                .inner_join(songs)
                .select((song_id, artist, title, genre, url, matched_at))
                .load::<HistoryEntry>(&self.0)?
        } else {
            history
                .inner_join(songs)
                .select((song_id, artist, title, genre, url, matched_at))
                .load::<HistoryEntry>(&self.0)?
        };

        Ok(entries)
    }
}

impl Message for GetMostPopular {
    type Result = Result<Vec<TopSong>, Error>;
}

impl Handler<GetMostPopular> for DbExecutor {
    type Result = Result<Vec<TopSong>, Error>;

    fn handle(&mut self, msg: GetMostPopular, _: &mut Self::Context) -> Self::Result {
        use super::schema::songs::dsl::{featured, songs};

        let mut featured_songs = songs
            .filter(featured.eq(true))
            .load::<Song>(&self.0)?
            .into_iter()
            .map(|s| TopSong {
                id: s.id,
                artist: s.artist,
                title: s.title,
                genre: s.genre,
                url: s.url,
                cnt: 0,
            })
            .collect::<Vec<_>>();

        let limit = (msg.limit as usize).saturating_sub(featured_songs.len());

        let mut entries = diesel::sql_query(
            "SELECT id, artist, title, genre, url, (
                SELECT count(song_id) FROM history WHERE songs.id = song_id
            ) cnt
            FROM songs
            WHERE featured = FALSE
            ORDER BY cnt DESC
            LIMIT ?;",
        )
        .bind::<Integer, _>(limit as i32)
        .load::<TopSong>(&self.0)?;

        featured_songs.append(&mut entries);

        Ok(featured_songs)
    }
}

impl Message for GetAllSongs {
    type Result = Result<Vec<Song>, Error>;
}

impl Handler<GetAllSongs> for DbExecutor {
    type Result = Result<Vec<Song>, Error>;

    fn handle(&mut self, msg: GetAllSongs, _: &mut Self::Context) -> Self::Result {
        use super::schema::songs::dsl::{artist, featured, genre, songs};

        let mut query = songs.into_boxed();

        if !msg.artists.is_empty() {
            query = query.filter(artist.eq_any(msg.artists));
        }

        if !msg.genres.is_empty() {
            query = query.filter(genre.eq_any(msg.genres));
        }

        if let Some(is_featured) = msg.featured {
            query = query.filter(featured.eq(is_featured));
        }

        Ok(query.load(&self.0)?)
    }
}

impl Message for EditSong {
    type Result = Result<(), Error>;
}

impl Handler<EditSong> for DbExecutor {
    type Result = Result<(), Error>;

    fn handle(&mut self, msg: EditSong, _: &mut Self::Context) -> Self::Result {
        match msg.song.save_changes::<Song>(&self.0) {
            Ok(_) => Ok(()),
            Err(diesel::result::Error::NotFound) => Err(Error::NotFound),
            Err(e) => Err(Error::DbError(e)),
        }
    }
}

impl Message for AddSong {
    type Result = Result<Song, Error>;
}

impl Handler<AddSong> for DbExecutor {
    type Result = Result<Song, Error>;

    fn handle(&mut self, msg: AddSong, _: &mut Self::Context) -> Self::Result {
        use super::schema::songs::dsl::{id, songs};

        diesel::insert_into(songs).values(&msg).execute(&self.0)?;

        Ok(songs.order(id.desc()).first(&self.0)?)
    }
}

impl Message for GetAllGenres {
    type Result = Result<Vec<String>, Error>;
}

impl Handler<GetAllGenres> for DbExecutor {
    type Result = Result<Vec<String>, Error>;

    fn handle(&mut self, _msg: GetAllGenres, _: &mut Self::Context) -> Self::Result {
        use super::schema::songs::dsl::{genre, songs};

        let entries = songs.select(genre).distinct().load(&self.0)?;

        Ok(entries)
    }
}

impl Message for GetAllArtists {
    type Result = Result<Vec<String>, Error>;
}

impl Handler<GetAllArtists> for DbExecutor {
    type Result = Result<Vec<String>, Error>;

    fn handle(&mut self, _msg: GetAllArtists, _: &mut Self::Context) -> Self::Result {
        use super::schema::songs::dsl::{artist, songs};

        let entries = songs.select(artist).distinct().load(&self.0)?;

        Ok(entries)
    }
}

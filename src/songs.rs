use actix_web::client::{self as awc, Client};
use actix_web::error::{ErrorBadRequest, ErrorForbidden, ErrorInternalServerError};
use actix_web::http::StatusCode;
use actix_web::web::{Bytes, Data, Json, Path};
use actix_web::Error;
use chrono::NaiveDateTime;
use diesel::{
    sql_types::{Integer, Text},
    Queryable,
};
use futures::{
    future::{self, Either},
    Future,
};
use serde::{Deserialize, Serialize};

use crate::auth::Auth;
use crate::db::songs::{
    EditSong, GetAllArtists, GetAllGenres, GetHistory, GetMostPopular, Recognize,
};
use crate::{Actors, Config};

pub use crate::db::models::Song;
pub use crate::db::songs::{AddSong, GetAllSongs as SongsFilter};
use crate::db::DbExecutor;
use actix::Addr;
use actix_web::dev::Body;
use futures::stream::Stream;
use std::time::Duration;

// TODO: check format
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SongRef {
    pub id: i32,
    pub mse: f64,
}

#[derive(Debug, Serialize, Queryable)]
pub struct HistoryEntry {
    pub id: i32,
    pub artist: String,
    pub title: String,
    pub genre: String,
    pub url: String,
    /// ISO 8601 / RFC 3339 format
    pub timestamp: NaiveDateTime,
}

#[derive(Debug, Serialize, QueryableByName)]
pub struct TopSong {
    #[sql_type = "Integer"]
    pub id: i32,
    #[sql_type = "Text"]
    pub artist: String,
    #[sql_type = "Text"]
    pub title: String,
    #[sql_type = "Text"]
    pub genre: String,
    #[sql_type = "Text"]
    pub url: String,
    /// How many times song was recognized. If song is featured, this always will be 0.
    #[sql_type = "Integer"]
    pub cnt: i32,
}

/// `POST /recognize/{n}`
///
/// Rozpoznaje utwór przesłany w body i zwraca informacje o `n` najbardziej podobnych utworów w
/// bazie. Zwraca Bad Request w przypadku nieprawidłowego formatu pliku dźwiękowego.
pub fn recognize(
    limit: Path<u32>,
    body: Bytes,
    auth: Option<Auth>,
    actors: Data<Actors>,
    config: Data<Config>,
) -> impl Future<Item = Json<Vec<Song>>, Error = Error> {
    Client::new()
        .post(format!(
            "{}/recognize?numberOfSongs={}",
            config.populator, *limit
        ))
        .timeout(Duration::from_secs(60))
        .send_body(body)
        .from_err::<Error>()
        .and_then(|mut res| {
            if !res.status().is_success() {
                if res.status() == StatusCode::BAD_REQUEST {
                    Either::A(future::err(ErrorBadRequest("Probably invalid format")))
                } else {
                    Either::A(future::err(ErrorInternalServerError("")))
                }
            } else {
                Either::B(res.json::<Vec<SongRef>>().from_err())
            }
        })
        .and_then(move |songs| {
            let msg = Recognize {
                song_ids: songs.iter().map(|s| s.id).collect(),
                user_id: auth.map(|a| a.id),
            };

            actors.db.send(msg).map_err(ErrorInternalServerError)
        })
        .and_then(|res| res.map_err(Error::from))
        .map(Json)
}

/// `GET /history`
///
/// Zwraca historię wyszukiwania używkonika.
pub fn history(
    auth: Auth,
    actors: Data<Actors>,
) -> impl Future<Item = Json<Vec<HistoryEntry>>, Error = Error> {
    let msg = GetHistory {
        user_id: Some(auth.id),
    };

    actors
        .db
        .send(msg)
        .map_err(ErrorInternalServerError)
        .and_then(|r| r.map_err(Error::from).map(Json))
}

/// `GET /history/all`
///
/// Zwraca całą historię wyszukiwania. Wymaga uprawnień administratora.
pub fn history_all(
    auth: Auth,
    actors: Data<Actors>,
) -> impl Future<Item = Json<Vec<HistoryEntry>>, Error = Error> {
    if !auth.is_admin {
        Either::A(future::err(ErrorForbidden("not admin")))
    } else {
        let msg = GetHistory { user_id: None };

        Either::B(
            actors
                .db
                .send(msg)
                .map_err(ErrorInternalServerError)
                .and_then(|r| r.map_err(Error::from).map(Json)),
        )
    }
}

/// `GET /popular/{n}`
///
/// Zwraca `n` najczęściej wyszukiwanych utworów.
pub fn popular(
    limit: Path<u32>,
    actors: Data<Actors>,
) -> impl Future<Item = Json<Vec<TopSong>>, Error = Error> {
    let msg = GetMostPopular { limit: *limit };

    actors
        .db
        .send(msg)
        .map_err(ErrorInternalServerError)
        .and_then(|r| r.map_err(Error::from).map(Json))
}

/// `POST /songs`
///
/// Zwraca wszystkie utwory.
pub fn songs(
    filter: Json<SongsFilter>,
    actors: Data<Actors>,
) -> impl Future<Item = Json<Vec<Song>>, Error = Error> {
    let msg = filter.into_inner();

    actors
        .db
        .send(msg)
        .map_err(ErrorInternalServerError)
        .and_then(|r| r.map_err(Error::from).map(Json))
}

/// `POST /edit_song`
///
/// Zastępuje metadane utworu nowymi. Jeśli ID jest nieprawidłowe, zwraca Not Found.
pub fn edit_song(
    song: Json<Song>,
    auth: Auth,
    actors: Data<Actors>,
) -> impl Future<Item = (), Error = Error> {
    if !auth.is_admin {
        Either::A(future::err(ErrorForbidden("not admin")))
    } else {
        let msg = EditSong {
            song: song.into_inner(),
        };

        Either::B(
            actors
                .db
                .send(msg)
                .map_err(ErrorInternalServerError)
                .and_then(|r| r.map_err(Error::from)),
        )
    }
}

/// `POST /add_song`
///
/// Dodaje nowy utwór. Zwraca BadRequest jeśli nie uda się pobrać pliku z podanego URLa. Wymaga
/// uprawnień administratora.
pub fn add_song(
    songs: Json<Vec<AddSong>>,
    auth: Auth,
    actors: Data<Actors>,
    config: Data<Config>,
) -> impl Future<Item = (), Error = Error> {
    if !auth.is_admin {
        Either::A(future::err(ErrorForbidden("not admin")))
    } else {
        let db = actors.db.clone();
        let populator_url = config.populator.clone();

        Either::B(
            future::join_all(
                songs
                    .into_inner()
                    .into_iter()
                    .map(move |song| send_song(song, populator_url.clone(), db.clone())),
            )
            .and_then(|_| future::ok(())),
        )
    }
}

fn send_song(
    song: AddSong,
    populator_url: String,
    db: Addr<DbExecutor>,
) -> impl Future<Item = (), Error = Error> {
    // It would be so much better with unstable await…
    // Use reqwest because awc doesn't follow 303
    reqwest::r#async::Client::new()
        .get(&song.url)
        .send()
        .map_err(ErrorBadRequest)
        .and_then(|res| res.error_for_status().map_err(ErrorBadRequest))
        .and_then(|res| res.into_body().concat2().map_err(ErrorBadRequest))
        .map(|body| Body::from_slice(&body))
        .and_then(move |data| {
            db.send(song)
                .map_err(ErrorInternalServerError)
                .and_then(|r| r.map_err(Error::from))
                .and_then(|metadata| {
                    let populator_url = populator_url;

                    awc::ClientBuilder::new()
                        .timeout(Duration::from_secs(120))
                        .finish()
                        .post(format!(
                            "{}/initialization/addSong/{}",
                            populator_url, metadata.id
                        ))
                        .send_body(data)
                        .from_err::<Error>()
                        .and_then(|res| {
                            if !res.status().is_success() {
                                Either::A(future::err(ErrorInternalServerError(format!(
                                    "Failed to upload song to populator, server returned {}",
                                    res.status()
                                ))))
                            } else {
                                Either::B(future::ok(()))
                            }
                        })
                })
        })
}

/// `GET /genres`
///
/// Zwraca wszystkie gatunki.
pub fn genres(actors: Data<Actors>) -> impl Future<Item = Json<Vec<String>>, Error = Error> {
    let msg = GetAllGenres;

    actors
        .db
        .send(msg)
        .map_err(ErrorInternalServerError)
        .and_then(|r| r.map_err(Error::from).map(Json))
}

/// `GET /artists`
///
/// Zwraca wszystkich wykonawców.
pub fn artists(actors: Data<Actors>) -> impl Future<Item = Json<Vec<String>>, Error = Error> {
    let msg = GetAllArtists;

    actors
        .db
        .send(msg)
        .map_err(ErrorInternalServerError)
        .and_then(|r| r.map_err(Error::from).map(Json))
}

use actix_web::error::{ErrorForbidden, ErrorInternalServerError};
use actix_web::web::{Data, Json};
use actix_web::Error;
use futures::{
    future::{self, Either},
    Future,
};
use serde::Serialize;

use crate::auth::Auth;
use crate::db::logs::GetLogs;
use crate::Actors;
use chrono::NaiveDateTime;

#[derive(Debug, Serialize)]
pub struct LogEntry {
    pub id: i32,
    pub login: String,
    /// ISO 8601 / RFC 3339 format
    pub logging_time: NaiveDateTime,
    pub logging_succession: bool,
    pub ip_addr: String,
    pub os: String,
    pub browser: String,
}

/// `GET /logs`
///
/// Zwraca logi użytkownika.
pub fn logs(
    auth: Auth,
    actors: Data<Actors>,
) -> impl Future<Item = Json<Vec<LogEntry>>, Error = Error> {
    if !auth.is_admin {
        Either::A(future::err(ErrorForbidden("not admin")))
    } else {
        let msg = GetLogs;

        Either::B(
            actors
                .db
                .send(msg)
                .map_err(ErrorInternalServerError)
                .and_then(|r| r.map_err(Error::from).map(Json)),
        )
    }
}

use actix::prelude::*;
use diesel::sqlite::SqliteConnection;

pub mod auth;
pub mod logs;
pub mod models;
pub mod schema;
pub mod songs;

pub struct DbExecutor(pub SqliteConnection);

impl Actor for DbExecutor {
    type Context = SyncContext<Self>;
}

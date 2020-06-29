use super::schema::{history, logs, songs, users};
use chrono::NaiveDateTime;
use diesel::{Insertable, Queryable};
use serde::{Deserialize, Serialize};

#[derive(Clone, Queryable, Debug, Serialize)]
pub struct User {
    pub id: i32,
    pub login: String,
    #[serde(skip)]
    /// Hash is NOT serialized.
    pub hash: String,
    pub role: String,
    pub active: bool,
}

impl User {
    pub const ROLE_CUSTOMER: &'static str = "CUSTOMER";
    pub const ROLE_ADMIN: &'static str = "ADMIN";
}

#[derive(Clone, Insertable, Debug)]
#[table_name = "users"]
pub struct NewUser<'a> {
    pub login: &'a str,
    pub hash: &'a str,
    pub role: &'a str,
}

#[derive(Clone, Queryable, Debug, Serialize, Deserialize, AsChangeset, Identifiable)]
pub struct Song {
    pub id: i32,
    pub artist: String,
    pub title: String,
    pub genre: String,
    pub url: String,
    pub featured: bool,
}

#[derive(Clone, Queryable, Debug)]
pub struct History {
    pub id: i32,
    pub song_id: i32,
    pub user_id: Option<i32>,
    pub matched_at: NaiveDateTime,
}

#[derive(Clone, Insertable, Debug)]
#[table_name = "history"]
pub struct NewHistory {
    pub song_id: i32,
    pub user_id: Option<i32>,
    pub matched_at: NaiveDateTime,
}

#[derive(Clone, Insertable, Debug)]
#[table_name = "logs"]
pub struct UserLog<'a> {
    pub login: &'a str,
    pub logging_time: NaiveDateTime,
    pub logging_succession: bool,
    pub ip_addr: &'a str,
    pub user_agent: &'a str,
}

#[derive(Debug, Queryable)]
pub struct Log {
    pub id: i32,
    pub login: String,
    /// ISO 8601 / RFC 3339 format
    pub logging_time: NaiveDateTime,
    pub logging_succession: bool,
    pub ip_addr: String,
    pub user_agent: String,
}

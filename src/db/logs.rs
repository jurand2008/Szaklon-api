use actix::prelude::*;
use actix_web::{dev::Body, http::StatusCode, web::HttpResponse, ResponseError};
use diesel::prelude::*;
use failure_derive::Fail;
use lazy_static::lazy_static;
use uaparser::{Parser as _, UserAgentParser};

use crate::db::models::Log;
use crate::db::DbExecutor;
use crate::logs::LogEntry;

lazy_static! {
    static ref UA_PARSER: UserAgentParser =
        UserAgentParser::from_bytes(include_bytes!("../../ua_regexes.yaml"))
            .expect("Invalid ua_regexes.yaml file");
}

pub struct GetLogs;

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "Database error: {}", _0)]
    DbError(#[cause] diesel::result::Error),
}

impl ResponseError for Error {
    fn error_response(&self) -> HttpResponse<Body> {
        match self {
            Error::DbError(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
}

impl From<diesel::result::Error> for Error {
    fn from(f: diesel::result::Error) -> Self {
        Error::DbError(f)
    }
}

impl Message for GetLogs {
    type Result = Result<Vec<LogEntry>, Error>;
}

impl Handler<GetLogs> for DbExecutor {
    type Result = Result<Vec<LogEntry>, Error>;

    fn handle(&mut self, _msg: GetLogs, _: &mut Self::Context) -> Self::Result {
        use super::schema::logs::dsl::logs;

        let vlog = logs.load::<Log>(&self.0)?;

        Ok(vlog
            .into_iter()
            .map(|log| {
                let ua = UA_PARSER.parse(&log.user_agent);
                let os = format!("{} {}", ua.os.family, ua.os.major.unwrap_or_default());
                let browser = format!(
                    "{} {}",
                    ua.user_agent.family,
                    ua.user_agent.major.unwrap_or_default()
                );

                LogEntry {
                    id: log.id,
                    login: log.login,
                    logging_time: log.logging_time,
                    logging_succession: log.logging_succession,
                    ip_addr: log.ip_addr,
                    os,
                    browser,
                }
            })
            .collect())
    }
}

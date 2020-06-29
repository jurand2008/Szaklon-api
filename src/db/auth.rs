use actix::prelude::*;
use diesel::prelude::*;
use failure_derive::Fail;
use unicode_normalization::UnicodeNormalization;

use crate::db::models::{NewUser, User, UserLog};
use crate::db::DbExecutor;
use crate::utils::PerfLog;
use actix_web::{dev::Body, http::StatusCode, web::HttpResponse, ResponseError};
use rand::Rng;

const HASH_CONFIG: argon2::Config = argon2::Config {
    ad: &[],
    hash_length: 32,
    lanes: 1,
    mem_cost: 128 * 1024, // 128 MiB
    secret: &[],
    thread_mode: argon2::ThreadMode::Sequential,
    time_cost: 2,
    variant: argon2::Variant::Argon2i,
    version: argon2::Version::Version13,
};

pub struct CreateUser {
    pub name: String,
    pub password: Vec<u8>,
}

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "Incorrect username or password")]
    InvalidCredentials,
    #[fail(display = "User already exists")]
    UserExists,
    #[fail(display = "Requested user not found")]
    NotFound,
    #[fail(display = "Database error occurred")]
    DbError(#[cause] diesel::result::Error),
    #[fail(display = "Error while hashing")]
    HashError(#[cause] argon2::Error),
}

impl ResponseError for Error {
    fn error_response(&self) -> HttpResponse<Body> {
        match self {
            Error::InvalidCredentials => HttpResponse::new(StatusCode::BAD_REQUEST),
            Error::UserExists => HttpResponse::new(StatusCode::BAD_REQUEST),
            Error::NotFound => HttpResponse::new(StatusCode::NOT_FOUND),
            Error::DbError(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
            Error::HashError(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
}

impl From<diesel::result::Error> for Error {
    fn from(f: diesel::result::Error) -> Self {
        Error::DbError(f)
    }
}

impl From<argon2::Error> for Error {
    fn from(f: argon2::Error) -> Self {
        Error::HashError(f)
    }
}

impl Message for CreateUser {
    type Result = Result<(), Error>;
}

impl Handler<CreateUser> for DbExecutor {
    type Result = Result<(), Error>;

    fn handle(&mut self, msg: CreateUser, _: &mut Self::Context) -> Self::Result {
        use super::schema::users::dsl::{self, users};

        let mut salt = [0u8; 16];
        rand::thread_rng().fill(&mut salt);

        let username = normalize_username(&msg.name);

        // Check if user already exists
        match users
            .filter(dsl::login.eq(&username))
            .first::<User>(&self.0)
        {
            Err(diesel::result::Error::NotFound) => (),
            Ok(_) => return Err(Error::UserExists),
            Err(e) => return Err(Error::DbError(e)),
        }

        let p = PerfLog::new();
        let hash = &argon2::hash_encoded(&msg.password, &salt, &HASH_CONFIG)?;
        p.log("Hash time");

        let new_user = NewUser {
            login: &username,
            hash,
            role: User::ROLE_CUSTOMER,
        };

        diesel::insert_into(users)
            .values(&new_user)
            .execute(&self.0)?;

        Ok(())
    }
}

pub struct Login {
    pub name: String,
    pub password: Vec<u8>,
    pub ip_addr: String,
    pub user_agent: String,
}

impl Message for Login {
    type Result = Result<User, Error>;
}

impl Handler<Login> for DbExecutor {
    type Result = Result<User, Error>;

    fn handle(&mut self, msg: Login, _: &mut Self::Context) -> Self::Result {
        use super::schema::logs::dsl::logs;
        use super::schema::users::dsl::{self, users};

        let username = normalize_username(&msg.name);

        let mut log_entry = UserLog {
            login: &username,
            logging_time: chrono::offset::Utc::now().naive_utc(),
            logging_succession: false,
            ip_addr: &msg.ip_addr,
            user_agent: &msg.user_agent,
        };

        let user = match users
            .filter(dsl::login.eq(&username))
            .first::<User>(&self.0)
        {
            Ok(u) => u,
            Err(diesel::result::Error::NotFound) => {
                diesel::insert_into(logs)
                    .values(&log_entry)
                    .execute(&self.0)?;

                return Err(Error::InvalidCredentials);
            }
            Err(e) => return Err(Error::DbError(e)),
        };

        log_entry.logging_succession =
            user.active && argon2::verify_encoded(&user.hash, &msg.password)?;

        diesel::insert_into(logs)
            .values(&log_entry)
            .execute(&self.0)?;

        if log_entry.logging_succession {
            Ok(user)
        } else {
            Err(Error::InvalidCredentials)
        }
    }
}

pub struct DeleteAccount {
    pub id: i32,
}

impl Message for DeleteAccount {
    type Result = Result<(), Error>;
}

impl Handler<DeleteAccount> for DbExecutor {
    type Result = Result<(), Error>;

    fn handle(&mut self, msg: DeleteAccount, _: &mut Self::Context) -> Self::Result {
        use super::schema::users::dsl::{active, id, users};

        match diesel::update(users.filter(id.eq(msg.id)))
            .set(active.eq(false))
            .execute(&self.0)
        {
            Ok(_) => Ok(()),
            Err(diesel::result::Error::NotFound) => Err(Error::NotFound),
            Err(e) => Err(Error::DbError(e)),
        }
    }
}

pub struct GetUsers;

impl Message for GetUsers {
    type Result = Result<Vec<User>, Error>;
}

impl Handler<GetUsers> for DbExecutor {
    type Result = Result<Vec<User>, Error>;

    fn handle(&mut self, _msg: GetUsers, _: &mut Self::Context) -> Self::Result {
        use super::schema::users::dsl::users;

        Ok(users.load::<User>(&self.0)?)
    }
}

fn normalize_username(s: &str) -> String {
    s.nfkc().collect::<String>().to_lowercase()
}

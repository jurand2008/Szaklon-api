use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{
    dev::Payload,
    error::{ErrorBadRequest, ErrorForbidden, ErrorInternalServerError, ErrorUnauthorized},
    web::{Data, Json},
    Error, FromRequest, HttpRequest,
};
use futures::{future, Future};
use log::error;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::db::auth::GetUsers;
use crate::db::auth::{CreateUser, DeleteAccount, Login};
use crate::Actors;
use futures::future::Either;

pub use crate::db::models::User;
use actix_web::web::Path;

const TOKEN_SIZE: usize = 32;

pub type Sessions = Arc<Mutex<HashMap<[u8; 32], User>>>;

/// Dane potrzebne do przeprowadzenia logowania.
#[derive(Debug, Deserialize)]
pub struct LoginData {
    pub login: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub is_admin: bool,
}

/// `POST /login`
///
/// Zaloguj się. Zwraca token który należy wrzucić do nagłówka Authorization. W przypadku
/// nieprawidłowej nazwy użytkownika lub hasła, albo złego formatu zapytania zwraca BadRequest.
pub fn login(
    login: Json<LoginData>,
    data: Data<Sessions>,
    actors: Data<Actors>,
    request: HttpRequest,
) -> impl Future<Item = Json<LoginResponse>, Error = Error> {
    let login = login.into_inner();
    let msg = Login {
        name: login.login,
        password: login.password.into(),
        ip_addr: request
            .peer_addr()
            .map(|addr| addr.ip().to_string())
            .unwrap_or(String::new()),
        user_agent: request
            .headers()
            .get("User-Agent")
            .and_then(|h| h.to_str().ok())
            .map(String::from)
            .unwrap_or(String::new()),
    };

    let sessions = (*data).clone();

    actors
        .db
        .send(msg)
        .map_err(|x| {
            error!("{}", x);

            ErrorInternalServerError("")
        })
        .and_then(move |res| match res {
            Ok(user) => {
                let mut buf = [0u8; TOKEN_SIZE];
                let mut rng = rand::thread_rng();
                rng.fill(&mut buf);
                sessions.lock().insert(buf, user.clone());

                Ok(Json(LoginResponse {
                    token: base64::encode(&buf),
                    is_admin: user.role == User::ROLE_ADMIN,
                }))
            }
            Err(e) => Err(e.into()),
        })
}

/// `POST /signup`
///
/// Tworzy nowego uzytkownika. W przypadku złego formatu zapytania lub gdy użytkownik z identyczną
/// nazwą istnieje zwraca BadRequest.
pub fn signup(
    login: Json<LoginData>,
    actors: Data<Actors>,
) -> impl Future<Item = (), Error = Error> {
    let login = login.into_inner();
    let msg = CreateUser {
        name: login.login,
        password: login.password.into(),
    };

    actors
        .db
        .send(msg)
        .map_err(|x| {
            error!("{}", x);

            ErrorInternalServerError("")
        })
        .and_then(|res| res.map_err(Error::from))
}

/// `DELETE /account`
///
/// Usuwa aktualne konto użytkownika i jego sesję.
pub fn delete_account(
    auth: Auth,
    data: Data<Sessions>,
    actors: Data<Actors>,
) -> impl Future<Item = (), Error = Error> {
    let msg = DeleteAccount { id: auth.id };

    actors
        .db
        .send(msg)
        .map_err(ErrorInternalServerError)
        .and_then(|r| r.map_err(Error::from))
        .and_then(|_| logout(auth, data))
}

/// `DELETE /account/{id}`
///
/// Usuwa wybrane konto użytkownika i jego sesję, jeśłi istnieje.
pub fn delete_account_admin(
    id: Path<i32>,
    auth: Auth,
    data: Data<Sessions>,
    actors: Data<Actors>,
) -> impl Future<Item = (), Error = Error> {
    if !auth.is_admin {
        Either::A(future::err(ErrorForbidden("not admin")))
    } else {
        let msg = DeleteAccount { id: *id };

        Either::B(
            actors
                .db
                .send(msg)
                .map_err(ErrorInternalServerError)
                .and_then(|r| r.map_err(Error::from))
                .and_then(move |_| {
                    let mut lock = data.lock();
                    if let Some(session) = lock.iter().find(|(_, v)| v.id == *id).map(|(k, _)| *k) {
                        lock.remove(&session);
                    }

                    Ok(())
                }),
        )
    }
}

/// Ekstraktor danych uwierzytelniających.
///
/// Ekstraktor spodziewa się tokenu sesji w nagłówku Authorization. Jeśli token nie istnieje, lub
/// nagłówka nie ma w zapytaniu zwrócony zostanie błąd Unauthorized.
pub struct Auth {
    pub token: [u8; 32],
    pub is_admin: bool,
    pub username: String,
    pub id: i32,
}

impl FromRequest for Auth {
    type Error = Error;
    type Future = Result<Auth, Error>;
    type Config = ();

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let sessions: &Sessions = req.app_data().expect("Sessions data is not configured!");

        if let Some(token) = req.headers().get("Authorization") {
            let token = token.to_str().map_err(|e| ErrorBadRequest(e))?;
            let mut buf = [0u8; 32];
            base64::decode_config_slice(token, base64::STANDARD, &mut buf)
                .map_err(|e| ErrorBadRequest(e))?;

            if let Some(user) = sessions.lock().get(&buf) {
                Ok(Auth {
                    token: buf,
                    is_admin: user.role == User::ROLE_ADMIN,
                    username: user.login.clone(),
                    id: user.id,
                })
            } else {
                Err(ErrorUnauthorized("Session doesn't exist"))
            }
        } else {
            Err(ErrorUnauthorized("Missing Authorization header"))
        }
    }
}

/// `POST /logout`
///
/// Kończy aktualną sesję.
pub fn logout(auth: Auth, sessions: Data<Sessions>) -> Result<(), Error> {
    sessions.lock().remove(&auth.token);

    Ok(())
}

/// `GET /users`
///
/// Zwraca wszystkich użytkowników. Wymaga uprawnień administratora.
pub fn users(
    auth: Auth,
    actors: Data<Actors>,
) -> impl Future<Item = Json<Vec<User>>, Error = Error> {
    if !auth.is_admin {
        Either::A(future::err(ErrorForbidden("not admin")))
    } else {
        Either::B(
            actors
                .db
                .send(GetUsers)
                .map_err(ErrorInternalServerError)
                .and_then(|r| r.map_err(Error::from).map(Json)),
        )
    }
}

/// `GET /check_session`
///
/// Zwraca 200 jeśli sesja jest poprawna.
pub fn check_session(_: Auth) {}

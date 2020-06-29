//! # Jak czytać dokumentację API HTTP?
//!
//! Wszystkie funkcje które pełnią rolę endpointu mają w pierwszej linijce dokumentacji metodę i
//! adres endpointu. Aby dowiedzieć się co przyjmują należy zwrócić uwagę na argumenty funkcji.
//! Np. `Json<LoginData>` oznacza, że należy podać dane jako JSON zgodne z `LoginData`. `Data<T>`
//! oraz `HttpRequest` można zignorować, są to szczegóły implementacji serwera.
//!
//! Jeśli parametr jest "opakowany" w enum `Option<T>` oznacza, ze jest on opcjonalny. Np.
//! `Option<Auth>` oznacza, że nie trzeba podawać tokenu sesji, ale może brakować wtedy pewnej
//! funkcjonalności, jak zapisanie wyszukiwania do historii.
//!
//! Inne szczegóły będą raczej podane w tekście samej dokumentacji.

#[macro_use]
extern crate diesel;

use actix::{Addr, SyncArbiter};
use actix_web::web::PayloadConfig;
use actix_web::{middleware, web, App, HttpServer};
use diesel::prelude::{Connection, SqliteConnection};
use failure::ResultExt;
use log::error;
use parking_lot::Mutex;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

use crate::db::models::User;
use db::DbExecutor;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
use std::path::PathBuf;

pub mod auth;
mod db;
mod init;
pub mod logs;
pub mod routes;
pub mod songs;
mod utils;

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub populator: String,
    pub scraper: String,
    pub extractor: String,
    pub bind_addr: String,
    #[serde(default)]
    pub tls_private_key_file: Option<PathBuf>,
    #[serde(default)]
    pub tls_cert_chain_file: Option<PathBuf>,
    #[serde(default)]
    pub tls_bind_addr: Option<String>,
    pub db_path: String,
    pub db_threads: usize,
    pub max_song_size: usize,
    #[serde(default)]
    pub max_songs_to_train: Option<usize>,
}

pub struct Actors {
    db: Addr<DbExecutor>,
}

fn main() -> Result<(), failure::Error> {
    env_logger::init();

    let config: Config =
        toml::from_str(&std::fs::read_to_string("config.toml").context("config.toml is missing")?)?;

    let connection = SqliteConnection::establish(&config.db_path).expect("Failed to open connection to db");
    let _ = connection.transaction(|| {
        init::init(&config, &connection).map_err(|e| {
            error!("Failed to initzialize system: {}", e);

            diesel::result::Error::RollbackTransaction
        })
    });
    std::mem::drop(connection);

    let _sys = actix::System::new("szaklon");

    let sessions = Arc::new(Mutex::new(HashMap::<[u8; 32], User>::new()));

    let database_url = config.db_path.clone();
    let db_addr = SyncArbiter::start(config.db_threads, move || {
        DbExecutor(
            SqliteConnection::establish(&database_url).expect("Failed to open connection to db"),
        )
    });

    let c = config.clone();
    let mut srv_builder = HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .wrap(middleware::cors::Cors::new())
            .data(sessions.clone())
            .data(c.clone())
            .data(Actors {
                db: db_addr.clone(),
            })
            .service(web::resource("/login").route(web::post().to_async(auth::login)))
            .service(web::resource("/signup").route(web::post().to_async(auth::signup)))
            .service(web::resource("/account").route(web::delete().to_async(auth::delete_account)))
            .service(
                web::resource("/account/{id}")
                    .route(web::delete().to_async(auth::delete_account_admin)),
            )
            .service(
                web::resource("/recognize/{n}")
                    .data(PayloadConfig::new(c.max_song_size))
                    .route(web::post().to_async(songs::recognize)),
            )
            .service(web::resource("/history").route(web::get().to_async(songs::history)))
            .service(web::resource("/history/all").route(web::get().to_async(songs::history_all)))
            .service(web::resource("/popular/{n}").route(web::get().to_async(songs::popular)))
            .service(web::resource("/songs").route(web::post().to_async(songs::songs)))
            .service(web::resource("/edit_song").route(web::post().to_async(songs::edit_song)))
            .service(web::resource("/add_song").route(web::post().to_async(songs::add_song)))
            .service(web::resource("/genres").route(web::get().to_async(songs::genres)))
            .service(web::resource("/artists").route(web::get().to_async(songs::artists)))
            .service(web::resource("/users").route(web::get().to_async(auth::users)))
            .service(web::resource("/check_session").route(web::get().to(auth::check_session)))
            .service(web::resource("/logs").route(web::get().to_async(logs::logs)))
            .route("/logout", web::post().to(auth::logout))
    })
    .bind(&config.bind_addr)?;

    // Enable TLS if configured
    if let (Some(key_path), Some(cert_path), Some(addr)) = (
        config.tls_private_key_file,
        config.tls_cert_chain_file,
        config.tls_bind_addr,
    ) {
        let mut tls_builder = SslAcceptor::mozilla_modern(SslMethod::tls()).unwrap();
        tls_builder
            .set_private_key_file(key_path, SslFiletype::PEM)
            .unwrap();
        tls_builder.set_certificate_chain_file(cert_path).unwrap();

        srv_builder = srv_builder.bind_ssl(addr, tls_builder)?;
    }

    println!("Listinig on:");
    for (addr, scheme) in srv_builder.addrs_with_scheme() {
        println!("{}://{}", scheme, addr);
    }

    srv_builder.run()?;

    Ok(())
}

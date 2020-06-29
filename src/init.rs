use crate::db::models::Song;
use crate::db::schema::songs;
use crate::utils::PerfLog;
use crate::Config;
use diesel::prelude::*;
use failure::format_err;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::ClientBuilder;
use serde::{Deserialize, Serialize};
use std::thread;
use std::time::Duration;

#[derive(Deserialize)]
struct Status {
    #[allow(dead_code)]
    n_features: i32,
    trained: bool,
}

#[derive(Deserialize)]
struct TrainResponse {
    task_id: String,
}

#[derive(Deserialize, Debug)]
struct TaskProgress {
    processed: u64,
    state: String,
    status: String,
    #[allow(dead_code)]
    total: u64,
}

#[derive(Deserialize, Insertable)]
#[table_name = "songs"]
struct ScraperGet {
    artist: String,
    title: String,
    genre: String,
    url: String,
}

#[derive(Serialize)]
struct PopulatorURL<'a> {
    id: i32,
    url: &'a str,
}

pub fn init(config: &Config, connector: &SqliteConnection) -> Result<(), failure::Error> {
    use crate::db::schema::songs::dsl::songs;

    if !songs.load::<Song>(connector)?.is_empty() {
        println!("Found some songs in database, skipping initialization.");

        return Ok(());
    }

    let client = ClientBuilder::new().timeout(None).build()?;

    let extractor_response: Status =
        reqwest::get(&format!("{}/status", config.extractor))?.json()?;
    if extractor_response.trained {
        return Ok(());
    }

    println!("Initializing system");

    println!(
        "{} Downloading metadata from scrapper…",
        console::style("[1/3]").bold()
    );

    let mut scraper_response: Vec<ScraperGet> = if let Some(max_songs) = config.max_songs_to_train {
        reqwest::get(&format!("{}/api/scrapper/{}", config.scraper, max_songs))?.json()?
    } else {
        reqwest::get(&format!("{}/api/scrapper", config.scraper))?.json()?
    };

    if let Some(max_songs) = config.max_songs_to_train {
        scraper_response.truncate(max_songs);
    }

    diesel::insert_into(songs)
        .values(&scraper_response)
        .execute(connector)?;

    let songs_vector: Vec<Song> = songs.load(connector)?;

    let urls: Vec<_> = songs_vector.iter().map(|song| &song.url).collect();

    println!(
        "{} Training extractor with {} songs…",
        console::style("[2/3]").bold(),
        urls.len()
    );

    let perf = PerfLog::new();
    let task_id = client
        .post(&format!("{}/train", config.extractor))
        .json(&urls)
        .send()?
        .json::<TrainResponse>()?
        .task_id;

    let pb = ProgressBar::new(urls.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})",
            )
            .progress_chars("#>-"),
    );
    pb.enable_steady_tick(50);

    loop {
        let task_progress: TaskProgress = client
            .get(&format!("{}/task_status/{}", config.extractor, task_id))
            .send()?
            .json()?;

        pb.set_position(task_progress.processed);

        match task_progress.state.as_str() {
            "PENDING" => (),
            "PROGRESS" => (),
            "SUCCESS" => break,
            _ => {
                return Err(format_err!(
                    "Error while training extractor: {}",
                    task_progress.status
                ))
            }
        }

        thread::sleep(Duration::from_secs(1));
    }

    pb.finish();

    perf.log("Training finished in");

    let urls: Vec<_> = songs_vector
        .iter()
        .map(|song| PopulatorURL {
            id: song.id,
            url: &song.url,
        })
        .collect();

    println!(
        "{} Initializing populator…",
        console::style("[3/3]").bold()
    );
    let perf = PerfLog::new();

    let send_urls_populator = client
        .post(&format!("{}/initialization", config.populator))
        .json(&urls)
        .send()?;
    if !send_urls_populator.status().is_success() {
        return Err(failure::err_msg("Error sending URL populator"));
    }

    perf.log("Populator init finished in");

    println!("Initialization completed.");

    Ok(())
}

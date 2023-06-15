use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use colored::Colorize;
use crossbeam::channel::Sender;
use reqwest::Client;
use tokio::{sync::Mutex, task::spawn};

use crate::{
    database::database::Database,
    log,
    parsers::{html::parse_html, json::parse_json, rss::parse_rss},
    structs::{Chapter, ParseMode, Target},
    CoreMessage,
};

pub async fn dispatch_gofers(
    database: Arc<Mutex<dyn Database>>,
    sender: Sender<CoreMessage>,
    targets: Vec<Target>,
) -> Result<()> {
    log!("{} Dispatching Gofers...", "[GOFR]".green());

    let mut handles = Vec::with_capacity(targets.len());

    for target in targets {
        let cloned_db_ref = database.clone();
        let handle = spawn(run_gofer(cloned_db_ref, target.clone()));
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.await;
    }

    log!("{} All Gofers have returned.", "[GOFR]".green());
    let _ = sender.send(CoreMessage::GoferFinished)?;
    Ok(())
}

pub async fn run_gofer(database: Arc<Mutex<dyn Database>>, target: Target) -> Result<()> {
    log!("{} Gofer started for {}...", "[GOFR]".green(), target.name);
    let mut chapters: Option<Vec<Chapter>> = None;

    let mut attempts = 5;
    while attempts > 0 {
        let fetch = fetch_chapters(&target).await;
        if fetch.is_ok() {
            chapters = Some(fetch.unwrap());
            break;
        } else {
            log!(
                "{} Gofer for {} encountered an error: {}",
                "[GOFR]".green(),
                target.name,
                fetch.unwrap_err()
            );
        }

        attempts -= 1;
    }

    if attempts == 0 {
        log!(
            "{} {}: Failed all fetching attempts.",
            "[GOFR]".green(),
            target.name,
        );
    }

    if chapters.is_some() {
        let chapters_ref = &chapters.unwrap();
        let mut attempts = 5;
        while attempts > 0 {
            let db = database.lock().await;
            let save = db.save_chapters(chapters_ref.as_slice());
            if save.is_ok() {
                break;
            }
            attempts -= 1;
        }

        if attempts == 0 {
            log!(
                "{} {}: Failed saving chapters.",
                "[GOFR]".green(),
                target.name,
            );
        }
    }

    log!("{} {}: Gofer finished.", "[GOFR]".green(), target.name);

    Ok(())
}

async fn fetch_chapters(target: &Target) -> Result<Vec<Chapter>> {
    let body = fetch_body(&target.source, &target.request_headers).await?;

    let chapters = match target.mode {
        ParseMode::Rss => parse_rss(target, &body)?,
        ParseMode::Json => parse_json(target, &body)?,
        ParseMode::Html => parse_html(target, &body)?,
    };

    Ok(chapters)
}

async fn fetch_body(url: &str, headers: &Option<HashMap<String, String>>) -> Result<String> {
    let client = Client::builder().gzip(true).brotli(true).build()?;
    let mut request = client.get(url);
    if headers.is_some() {
        for header in headers.as_ref().unwrap() {
            request = request.header(header.0, header.1);
        }
    }

    let response = request.send().await?;
    Ok(response.text().await?)
}

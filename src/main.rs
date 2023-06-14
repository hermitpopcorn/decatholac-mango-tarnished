use std::sync::Arc;

use announcer::dispatch_announcer;
use anyhow::Result;
use config::{get_config, get_targets};
use crossbeam::channel::{Receiver, Sender};
use database::sqlite::SqliteDatabase;
use discord::{connect_discord, get_discord_token};
use gofer::dispatch_gofers;
use poise::serenity_prelude::Http;
use tokio::{
    spawn,
    sync::Mutex,
    task::JoinHandle,
    time::{sleep, Duration},
};

mod announcer;
mod config;
mod database;
mod discord;
mod gofer;
mod parsers;
mod structs;
mod utils;

pub enum CoreMessage {
    StartGofer,
    GoferFinished,
    StartAnnouncer,
    AnnouncerFinished,
    StartDiscordBot,
    TransferDiscordHttp(Arc<Http>),
    Quit,
}

#[derive(PartialEq)]
enum Worker {
    Gofer,
    Announcer,
    DiscordBot,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Get config values
    let config = get_config(Some("settings.toml"))?;
    let targets = get_targets(config.get("targets"))?;
    let token = get_discord_token(config.get("token"))?;

    // Setup database
    let database = SqliteDatabase::new("database.db");
    let database_arc = Arc::new(Mutex::new(database));

    // Setup message channel for processes to communicate to core control (here)
    let (sender, receiver): (Sender<CoreMessage>, Receiver<CoreMessage>) =
        crossbeam::channel::unbounded();

    // Setup vector of processes to keep track what is running
    let mut handles: Vec<(Worker, JoinHandle<Result<()>>)> = vec![];

    // Setup memory storage for Discord API
    let mut discord_http: Option<Arc<Http>> = None;

    // One-shot toggle
    let mut boot = true;

    loop {
        if boot {
            let _ = sender.send(CoreMessage::StartGofer)?;
            let _ = sender.send(CoreMessage::StartDiscordBot)?;
            boot = false;
        }

        if let Ok(message) = receiver.recv() {
            match message {
                CoreMessage::StartGofer => {
                    if get_worker_index(&handles, Worker::Gofer).is_none() {
                        handles.push((
                            Worker::Gofer,
                            spawn(dispatch_gofers(
                                database_arc.clone(),
                                sender.clone(),
                                targets.clone(),
                            )),
                        ));
                        log!("[CORE] Tracking Gofer handle.");
                    }
                }
                CoreMessage::GoferFinished => {
                    let index = get_worker_index(&handles, Worker::Gofer);
                    if index.is_some() {
                        log!("[CORE] Removed Gofer handle.");
                        handles.remove(index.unwrap());
                    }

                    // Spawn another thread to wait a little and trigger announcer
                    let cloned_discord_http = discord_http.clone();
                    let cloned_sender = sender.clone();
                    spawn(async move {
                        sleep(Duration::from_millis(2500)).await;

                        if cloned_discord_http.is_some() {
                            cloned_sender.send(CoreMessage::StartAnnouncer).unwrap();
                        }
                    });
                }
                CoreMessage::StartAnnouncer => {
                    if discord_http.is_none() {
                        log!("[CORE] Could not start Announcer because Discord API has not been received by core control.");
                        continue;
                    }

                    if get_worker_index(&handles, Worker::Announcer).is_some() {
                        continue;
                    }

                    handles.push((
                        Worker::Announcer,
                        spawn(dispatch_announcer(
                            database_arc.clone(),
                            discord_http.clone().unwrap(),
                            sender.clone(),
                        )),
                    ));
                    log!("[CORE] Tracking Announcer handle.");
                }
                CoreMessage::AnnouncerFinished => {
                    let index = get_worker_index(&handles, Worker::Announcer);
                    if index.is_some() {
                        log!("[CORE] Removed Announcer handle.");
                        handles.remove(index.unwrap());
                    }
                }
                CoreMessage::StartDiscordBot => {
                    if get_worker_index(&handles, Worker::DiscordBot).is_none() {
                        log!("[CORE] Tracking DiscordBot handle.");
                        handles.push((
                            Worker::DiscordBot,
                            spawn(connect_discord(
                                database_arc.clone(),
                                sender.clone(),
                                token.clone(),
                            )),
                        ));
                    }
                }
                CoreMessage::TransferDiscordHttp(http) => {
                    discord_http = Some(http);
                    log!("[CORE] Discord API received.");
                }
                CoreMessage::Quit => {
                    break;
                }
            }
        }
    }

    Ok(())
}

fn get_worker_index(
    handles: &Vec<(Worker, JoinHandle<Result<()>>)>,
    what: Worker,
) -> Option<usize> {
    for (index, handle) in handles.iter().enumerate() {
        if handle.0 == what {
            return Some(index);
        }
    }

    None
}

use std::sync::Arc;

use anyhow::Result;
use config::{get_config, get_targets};
use crossbeam::channel::{Receiver, Sender};
use database::sqlite::SqliteDatabase;
use discord::{connect_discord, get_discord_token};
use gofer::dispatch_gofers;
use tokio::{spawn, sync::Mutex, task::JoinHandle};

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
    StartDiscordBot,
}

#[derive(PartialEq)]
enum Worker {
    Gofer,
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
    let mut handlers: Vec<(Worker, JoinHandle<Result<()>>)> = vec![];

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
                    if get_worker_index(&handlers, Worker::Gofer).is_none() {
                        handlers.push((
                            Worker::Gofer,
                            spawn(dispatch_gofers(
                                database_arc.clone(),
                                sender.clone(),
                                targets.clone(),
                            )),
                        ));
                        log!("Pushed Gofer into handlers.");
                    }
                }
                CoreMessage::GoferFinished => {
                    let index = get_worker_index(&handlers, Worker::Gofer);
                    if index.is_some() {
                        log!("Removed Gofer from handlers");
                        handlers.remove(index.unwrap());
                    }
                }
                CoreMessage::StartDiscordBot => {
                    if get_worker_index(&handlers, Worker::DiscordBot).is_none() {
                        log!("Pushed DiscordBot into handlers.");
                        handlers.push((
                            Worker::DiscordBot,
                            spawn(connect_discord(
                                database_arc.clone(),
                                sender.clone(),
                                token.clone(),
                            )),
                        ));
                    }
                }
            }
        }
    }

    Ok(())
}

fn get_worker_index(
    handlers: &Vec<(Worker, JoinHandle<Result<()>>)>,
    what: Worker,
) -> Option<usize> {
    for (index, handler) in handlers.iter().enumerate() {
        if handler.0 == what {
            return Some(index);
        }
    }

    None
}

use std::sync::Arc;

use announcer::{dispatch_announcer, dispatch_solo_announcer};
use anyhow::Result;
use colored::Colorize;
use config::{get_config, get_cron_schedule, get_discord_token, get_targets};
use crony::{Job, Runner, Schedule};
use crossbeam::channel::{Receiver, Sender};
use database::sqlite::SqliteDatabase;
use discord::{connect_discord, disconnect_discord};
use gofer::dispatch_gofers;
use poise::serenity_prelude::Http;
use structs::Server;
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

/// Enum of message types that will be sent from spawned threads back to the main thread.
pub enum CoreMessage {
    StartGofer(bool),
    GoferFinished(bool),
    StartAnnouncer,
    AnnouncerFinished,
    StartSoloAnnouncer(Server),
    SoloAnnouncerFinished(Server),
    StartDiscordBot,
    TransferDiscordHttp(Arc<Http>),
    Quit,
}

/// Types of workers.
#[derive(PartialEq)]
enum Worker {
    Gofer,
    Announcer,
    SoloAnnouncer(Server),
    DiscordBot,
}

impl std::fmt::Display for Worker {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Worker::Gofer => format!("Gofer"),
                Worker::Announcer => format!("Announcer"),
                Worker::SoloAnnouncer(server) =>
                    format!("Solo Announcer for {}", server.identifier),
                Worker::DiscordBot => format!("Discord Bot"),
            }
        )
    }
}

/// A cron that will send a message to the main thread to start up the Gofer worker periodically.
struct WorkerCron {
    schedule: Option<String>,
    sender: Sender<CoreMessage>,
}

impl Job for WorkerCron {
    /// The schedule will defer to the struct's `schedule` property,
    /// but if it failed to parse it, a sensible default (once every 12 PM)
    /// will be used instead.
    fn schedule(&self) -> Schedule {
        if self.schedule.as_ref().is_none() {
            return "0 0 12 * * *".parse().unwrap();
        }

        let schedule: Schedule = match self.schedule.as_ref().unwrap().parse() {
            Ok(s) => s,
            Err(_) => "0 0 12 * * *".parse().unwrap(),
        };

        schedule
    }
    fn handle(&self) {
        log!("{} WorkerCron handler triggered.", "[CORE]".blue());

        match self.sender.send(CoreMessage::StartGofer(true)) {
            Ok(_) => (),
            Err(_) => log!("{} Something went wrong with WorkerCron.", "[CORE]".blue()),
        };
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Get config values
    let config = get_config(Some("settings.toml"))?;
    let targets: Vec<structs::Target> = get_targets(config.get("targets"))?;
    let token = get_discord_token(config.get("token"))?;
    let cron_schedule = get_cron_schedule(config.get("cron"))?;

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

    // Run cron runner
    let mut runner = Runner::new();
    runner = runner.add(Box::new(WorkerCron {
        schedule: cron_schedule,
        sender: sender.clone(),
    }));
    runner = runner.run();

    // Create handler for termination signal
    let termination_sender = sender.clone();
    spawn(async {
        ctrlc::set_handler(move || {
            termination_sender
                .send(CoreMessage::Quit)
                .expect("Could not send termination signal on channel.")
        })
        .expect("Error setting Ctrl-C handler.");
    });

    // One-shot toggle
    let mut boot = true;

    loop {
        if boot {
            let _ = sender.send(CoreMessage::StartGofer(true))?;
            let _ = sender.send(CoreMessage::StartDiscordBot)?;
            boot = false;
        }

        if let Ok(message) = receiver.recv() {
            match message {
                CoreMessage::StartGofer(triggers_announcer) => {
                    if get_worker_index(&handles, Worker::Gofer).is_none() {
                        handles.push((
                            Worker::Gofer,
                            spawn(dispatch_gofers(
                                database_arc.clone(),
                                sender.clone(),
                                targets.clone(),
                                triggers_announcer,
                            )),
                        ));
                        log!("{} Tracking {} handle.", "[CORE]".blue(), Worker::Gofer);
                    }
                }
                CoreMessage::GoferFinished(triggers_announcer) => {
                    let index = get_worker_index(&handles, Worker::Gofer);
                    if index.is_some() {
                        log!("{} Removed {} handle.", "[CORE]".blue(), Worker::Gofer);
                        handles.remove(index.unwrap());
                    }

                    if triggers_announcer {
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
                }
                CoreMessage::StartAnnouncer => {
                    if discord_http.is_none() {
                        log!("{} Could not start {} because Discord API has not been received by core control.", "[CORE]".blue(), Worker::Announcer);
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
                    log!("{} Tracking {} handle.", "[CORE]".blue(), Worker::Announcer);
                }
                CoreMessage::AnnouncerFinished => {
                    let index = get_worker_index(&handles, Worker::Announcer);
                    if index.is_some() {
                        log!("{} Removed {} handle.", "[CORE]".blue(), Worker::Announcer);
                        handles.remove(index.unwrap());
                    }
                }
                CoreMessage::StartSoloAnnouncer(server) => {
                    if discord_http.is_none() {
                        log!("{} Could not start {} because Discord API has not been received by core control.", "[CORE]".blue(), Worker::Announcer);
                        continue;
                    }

                    handles.push((
                        Worker::SoloAnnouncer(server.clone()),
                        spawn(dispatch_solo_announcer(
                            database_arc.clone(),
                            discord_http.clone().unwrap(),
                            sender.clone(),
                            server,
                        )),
                    ));
                    log!("{} Tracking {} handle.", "[CORE]".blue(), Worker::Announcer);
                }
                CoreMessage::SoloAnnouncerFinished(server) => {
                    let index = get_worker_index(&handles, Worker::SoloAnnouncer(server.clone()));
                    if index.is_some() {
                        log!(
                            "{} Removed {} handle.",
                            "[CORE]".blue(),
                            Worker::SoloAnnouncer(server)
                        );
                        handles.remove(index.unwrap());
                    }
                }
                CoreMessage::StartDiscordBot => {
                    if get_worker_index(&handles, Worker::DiscordBot).is_none() {
                        log!(
                            "{} Tracking {} handle.",
                            "[CORE]".blue(),
                            Worker::DiscordBot,
                        );
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
                    log!("{} Discord API received.", "[CORE]".blue());
                }
                CoreMessage::Quit => {
                    break;
                }
            }
        }
    }

    runner.stop();

    for handle in handles {
        match handle.0 {
            Worker::Gofer | Worker::Announcer | Worker::SoloAnnouncer(_) => {
                handle.1.abort();
                log!("{} {} handle aborted.", "[CORE]".blue(), handle.0);
            }
            Worker::DiscordBot => {
                if discord_http.as_ref().is_some() {
                    let _ = disconnect_discord(discord_http.as_ref().unwrap()).await;
                }
                handle.1.abort();
                log!("{} {} handle aborted.", "[CORE]".blue(), Worker::DiscordBot);
            }
        };
    }

    log!("{} Goodbye!", "[CORE]".blue());
    Ok(())
}

/// Checks whether a worker already exists in the handle or not.
/// This is to keep the core control from starting multiple instances of the same worker.
/// The function returns the index wrapped in `Some` if it does, and `None` if it does not.
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

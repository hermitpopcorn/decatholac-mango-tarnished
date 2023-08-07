use std::sync::Arc;

use announcer::{dispatch_announcer, dispatch_solo_announcer};
use anyhow::{bail, Result};
use colored::Colorize;
use config::{get_config, get_cron_schedule, get_discord_token, get_targets};
use crony::{Job, Runner, Schedule};
use crossbeam::channel::{Receiver, Sender};
use database::{database::Database, sqlite::SqliteDatabase};
use discord::{connect_discord, disconnect_discord};
use gofer::dispatch_gofers;
use poise::serenity_prelude::Http;
use structs::{Server, Target};
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
#[derive(PartialEq, Clone)]
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
    schedule: Schedule,
    sender: Sender<CoreMessage>,
}

impl Job for WorkerCron {
    /// The schedule will defer to the struct's `schedule` property,
    /// but if it failed to parse it, a sensible default (once every 10 AM JST)
    /// will be used instead.
    fn schedule(&self) -> Schedule {
        self.schedule.clone()
    }
    fn handle(&self) {
        log!("{} WorkerCron handler triggered.", "[CORE]".blue());

        match self.sender.send(CoreMessage::StartGofer(true)) {
            Ok(_) => (),
            Err(_) => log!("{} Something went wrong with WorkerCron.", "[CORE]".blue()),
        };
    }
}

/// A vector of Worker and JoinHandle tuples.
type WorkerHandles = Vec<(Worker, JoinHandle<Result<()>>)>;

struct Flags {
    one_shot: bool,
}

impl Default for Flags {
    fn default() -> Self {
        Self { one_shot: false }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Get parameters
    let mut flags: Flags = Flags {
        ..Default::default()
    };
    let mut args: Vec<String> = std::env::args().collect();
    args.remove(0);
    for arg in args {
        match arg.as_str() {
            "--oneshot" | "--one-shot" | "-1s" => flags.one_shot = true,
            _ => continue,
        }
    }

    // Get config values
    let config = get_config(Some("settings.toml"))?;
    let targets: Vec<Target> = get_targets(config.get("targets"))?;
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

    // Run workers sequentially and terminate if one-shot flag is true
    if flags.one_shot {
        start_discord_bot(
            &mut handles,
            database_arc.clone(),
            sender.clone(),
            token.clone(),
        )
        .expect("one-shot: Start Discord Bot");
        let message = receiver.recv()?; // Await for Discord API
        match message {
            CoreMessage::TransferDiscordHttp(api) => discord_http = Some(api),
            _ => panic!("Unexpected response"),
        }
        start_gofer(
            &mut handles,
            database_arc.clone(),
            sender.clone(),
            targets.clone(),
            true,
        )
        .expect("one-shot: Start Gofer");
        receiver.recv()?; // Await for Gofer to finish
        start_announcer(
            &mut handles,
            database_arc.clone(),
            sender.clone(),
            discord_http.clone(),
            None,
        )
        .expect("one-shot: Start Announcer");
        receiver.recv()?; // Await for Announcer to finish
        disconnect_discord(discord_http.as_ref().unwrap())
            .await
            .expect("one-shot: Disconnect Discord");

        log!(
            "{} One-shot execution finished. Terminating.",
            "[CORE]".blue()
        );
        return Ok(());
    }

    // Run cron runner
    let mut runner = Runner::new();
    runner = runner.add(Box::new(WorkerCron {
        schedule: cron_schedule,
        sender: sender.clone(),
    }));
    runner = runner.run();

    // Create handler for termination signal
    let termination_sender = sender.clone();
    ctrlc::set_handler(move || {
        termination_sender
            .send(CoreMessage::Quit)
            .expect("Could not send termination signal on channel.")
    })
    .expect("Error setting Ctrl-C handler.");

    // Start-on-run toggle
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
                    let _ = start_gofer(
                        &mut handles,
                        database_arc.clone(),
                        sender.clone(),
                        targets.clone(),
                        triggers_announcer,
                    );
                }
                CoreMessage::GoferFinished(triggers_announcer) => {
                    let _ = remove_worker_handle(&mut handles, &Worker::Gofer);

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
                    let _ = start_announcer(
                        &mut handles,
                        database_arc.clone(),
                        sender.clone(),
                        discord_http.clone(),
                        None,
                    );
                }
                CoreMessage::AnnouncerFinished => {
                    let _ = remove_worker_handle(&mut handles, &Worker::Announcer);
                }
                CoreMessage::StartSoloAnnouncer(server) => {
                    let _ = start_announcer(
                        &mut handles,
                        database_arc.clone(),
                        sender.clone(),
                        discord_http.clone(),
                        Some(server),
                    );
                }
                CoreMessage::SoloAnnouncerFinished(server) => {
                    let _ = remove_worker_handle(&mut handles, &Worker::SoloAnnouncer(server));
                }
                CoreMessage::StartDiscordBot => {
                    let _ = start_discord_bot(
                        &mut handles,
                        database_arc.clone(),
                        sender.clone(),
                        token.clone(),
                    );
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
    what: &Worker,
) -> Option<usize> {
    for (index, handle) in handles.iter().enumerate() {
        if handle.0 == *what {
            return Some(index);
        }
    }

    None
}

/// Remove a worker from the handle list if it does exist in it.
fn remove_worker_handle(handles: &mut WorkerHandles, what: &Worker) -> Result<Option<usize>> {
    let index = get_worker_index(&handles, what);
    if index.is_none() {
        return Ok(None);
    }

    handles.remove(index.unwrap());
    log!("{} Removed {} handle.", "[CORE]".blue(), what);

    Ok(index)
}

/// Starts the Discord Bot worker and registers it into the handle list.
fn start_discord_bot(
    handles: &mut WorkerHandles,
    database_arc: Arc<Mutex<dyn Database>>,
    sender: Sender<CoreMessage>,
    token: String,
) -> Result<()> {
    if get_worker_index(handles, &Worker::DiscordBot).is_some() {
        bail!("Discord Bot is already running.");
    }

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

    Ok(())
}

/// Starts the Gofer worker and registers it into the handle list.
fn start_gofer(
    handles: &mut WorkerHandles,
    database_arc: Arc<Mutex<dyn Database>>,
    sender: Sender<CoreMessage>,
    targets: Vec<Target>,
    triggers_announcer: bool,
) -> Result<()> {
    if get_worker_index(&handles, &Worker::Gofer).is_some() {
        bail!("Gofer is already running.");
    }

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

    Ok(())
}

/// Starts the Announcer worker and registers it into the handle list.
fn start_announcer(
    handles: &mut WorkerHandles,
    database_arc: Arc<Mutex<dyn Database>>,
    sender: Sender<CoreMessage>,
    discord_http: Option<Arc<Http>>,
    server: Option<Server>,
) -> Result<()> {
    let worker = match server {
        Some(server) => Worker::SoloAnnouncer(server),
        None => Worker::Announcer,
    };

    if discord_http.is_none() {
        log!(
            "{} Could not start {} because Discord API has not been received by core control.",
            "[CORE]".blue(),
            worker,
        );
        bail!("Discord API has not been received by core control.");
    }
    if worker == Worker::Announcer && get_worker_index(&handles, &Worker::Announcer).is_some() {
        bail!("Announcer is already running.");
    }

    let discord_http = discord_http.unwrap();

    let handle = match worker.clone() {
        Worker::Announcer => (
            Worker::Announcer,
            spawn(dispatch_announcer(database_arc, discord_http, sender)),
        ),
        Worker::SoloAnnouncer(server) => (
            Worker::SoloAnnouncer(server.clone()),
            spawn(dispatch_solo_announcer(
                database_arc,
                discord_http,
                sender,
                server,
            )),
        ),
        _ => bail!("Invalid match on worker type check."),
    };
    handles.push(handle);
    log!("{} Tracking {} handle.", "[CORE]".blue(), worker);

    Ok(())
}

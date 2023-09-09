use std::sync::Arc;

use crate::database::{database::Database, sqlite::SqliteDatabase};
use announcer::{dispatch_announcer, dispatch_solo_announcer};
use anyhow::{bail, Result};
use colored::Colorize;
use config::{get_config, get_cron_schedule, get_discord_token, get_targets};
use crony::{Job, Runner, Schedule};
use crossbeam::channel::{Receiver, Sender};
use discord::{connect_discord, disconnect_discord};
use gofer::dispatch_gofers;
use poise::serenity_prelude::Http;
use structs::{Server, Target};
use tokio::{task::JoinSet, time::Duration};

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
    StartAnnouncer,
    StartSoloAnnouncer(Server),
    StartDiscordBot,
    TransferDiscordHttp(Arc<Http>),
    Quit,
}

/// Types of workers.
#[derive(PartialEq, Clone)]
pub enum Worker {
    Gofer,
    Announcer,
    SoloAnnouncer(Server),
    DiscordBot,
}

impl std::fmt::Display for Worker {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let str = match self {
            Worker::Gofer => format!("Gofer"),
            Worker::Announcer => format!("Announcer"),
            Worker::SoloAnnouncer(server) => format!("Solo Announcer for {}", server.identifier),
            Worker::DiscordBot => format!("Discord Bot"),
        };
        write!(f, "{}", str)
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

struct Flags {
    one_shot: bool,
}

impl Default for Flags {
    fn default() -> Self {
        Self { one_shot: false }
    }
}

type Handle = (Worker, Result<()>);

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
    let database = SqliteDatabase::new("database.db").await;
    let database_arc = Arc::new(database);

    // Setup message channel for processes to communicate to core control (here)
    let (sender, receiver): (Sender<CoreMessage>, Receiver<CoreMessage>) =
        crossbeam::channel::unbounded();

    // Setup vector of processes to keep track what is running
    let mut tracker: Vec<Worker> = vec![];
    let mut handles: JoinSet<Handle> = JoinSet::new();

    // Setup memory storage for Discord API
    let mut discord_http: Option<Arc<Http>> = None;

    // Run workers sequentially and terminate if one-shot flag is true
    if flags.one_shot {
        let exec = execute_one_shot(
            tracker,
            handles,
            database_arc,
            sender,
            receiver,
            token,
            targets,
        )
        .await;
        if let Err(error) = exec {
            log!("{} One-shot execution failed: {}", "[CORE]".blue(), error);
            return Err(error);
        }
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

    let mut trigger_announcer_on_gofer_finish = false;

    loop {
        if boot {
            sender.send(CoreMessage::StartGofer(true))?;
            sender.send(CoreMessage::StartDiscordBot)?;
            boot = false;
        }

        tokio::select! { biased;
            Some(finished_handle) = handles.join_next() => {
                // Continue on JoinError
                if let Err(join_error) = finished_handle {
                    log!("{} JoinError: {}.", "[CORE]".blue(), join_error);
                    continue;
                }
                let finished_handle = finished_handle.unwrap();

                // Remove from tracker
                let worker = finished_handle.0;
                if let Err(error) = remove_tracker(&mut tracker, &worker) {
                    log!("{} Error removing tracker: {}.", "[CORE]".blue(), error);
                }

                // Trigger Announcer if flag is true
                if worker == Worker::Gofer && trigger_announcer_on_gofer_finish {
                    trigger_announcer_on_gofer_finish = false;
                    sender.send(CoreMessage::StartAnnouncer)?
                }

                // Attempt restart if Discord Bot
                if worker == Worker::DiscordBot {
                    discord_http = None;
                    sender.send(CoreMessage::StartDiscordBot)?;
                }
            }

            message = async { receiver.try_recv() } => {
                // If no message, sleep for 100ms and continue loop
                if message.is_err() {
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }

                match message.unwrap() {
                    CoreMessage::StartGofer(triggers_announcer) => {
                        start_gofer(
                            &mut tracker,
                            &mut handles,
                            database_arc.clone(),
                            targets.clone(),
                        )?;

                        trigger_announcer_on_gofer_finish = triggers_announcer;
                    }
                    CoreMessage::StartAnnouncer => {
                        start_announcer(
                            &mut tracker,
                            &mut handles,
                            database_arc.clone(),
                            discord_http.clone(),
                            None,
                        )?;
                    }
                    CoreMessage::StartSoloAnnouncer(server) => {
                        start_announcer(
                            &mut tracker,
                            &mut handles,
                            database_arc.clone(),
                            discord_http.clone(),
                            Some(server),
                        )?;
                    }
                    CoreMessage::StartDiscordBot => {
                        start_discord_bot(
                            &mut tracker,
                            &mut handles,
                            database_arc.clone(),
                            sender.clone(),
                            token.clone(),
                        )?;
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
    }

    runner.stop();

    handles.abort_all();
    while let Some(_) = handles.join_next().await {
        // Loop until all handles have aborted
    }

    if tracker.contains(&Worker::DiscordBot) {
        disconnect_discord(discord_http.as_ref().unwrap()).await?;
    }

    log!("{} Goodbye!", "[CORE]".blue());
    Ok(())
}

/// Checks whether a worker already exists in the tracker or not.
/// This is to keep the core control from starting multiple instances of the same worker.
/// The function returns the index wrapped in `Some` if it does, and `None` if it does not.
fn get_tracker_index(tracker: &Vec<Worker>, find: &Worker) -> Option<usize> {
    for (index, worker) in tracker.iter().enumerate() {
        if *worker == *find {
            return Some(index);
        }
    }

    None
}

/// Add a worker to the tracker.
fn add_tracker(tracker: &mut Vec<Worker>, worker: Worker) -> Result<()> {
    tracker.push(worker);

    Ok(())
}

/// Remove a worker from the tracker if it does exist in it.
fn remove_tracker(tracker: &mut Vec<Worker>, worker: &Worker) -> Result<Option<usize>> {
    let index = get_tracker_index(&tracker, worker);
    if index.is_none() {
        return Ok(None);
    }

    tracker.remove(index.unwrap());

    Ok(index)
}

/// Starts the Discord Bot worker and registers it into the handle list.
fn start_discord_bot(
    tracker: &mut Vec<Worker>,
    handles: &mut JoinSet<Handle>,
    database_arc: Arc<dyn Database>,
    sender: Sender<CoreMessage>,
    token: String,
) -> Result<()> {
    if get_tracker_index(tracker, &Worker::DiscordBot).is_some() {
        bail!("Discord Bot is already running.");
    }

    add_tracker(tracker, Worker::DiscordBot)?;
    handles.spawn(connect_discord(
        database_arc.clone(),
        sender.clone(),
        token.clone(),
    ));

    Ok(())
}

/// Starts the Gofer worker and registers it into the handle list.
fn start_gofer(
    tracker: &mut Vec<Worker>,
    handles: &mut JoinSet<Handle>,
    database_arc: Arc<dyn Database>,
    targets: Vec<Target>,
) -> Result<()> {
    if get_tracker_index(&tracker, &Worker::Gofer).is_some() {
        bail!("Gofer is already running.");
    }

    add_tracker(tracker, Worker::Gofer)?;
    handles.spawn(dispatch_gofers(database_arc.clone(), targets.clone()));

    Ok(())
}

/// Starts the Announcer worker and registers it into the handle list.
fn start_announcer(
    tracker: &mut Vec<Worker>,
    handles: &mut JoinSet<Handle>,
    database_arc: Arc<dyn Database>,
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
    if get_tracker_index(&tracker, &worker).is_some() {
        bail!("Announcer is already running.");
    }

    let discord_http = discord_http.unwrap();

    match worker.clone() {
        Worker::Announcer => handles.spawn(dispatch_announcer(database_arc, discord_http)),
        Worker::SoloAnnouncer(server) => {
            handles.spawn(dispatch_solo_announcer(database_arc, discord_http, server))
        }
        _ => bail!("Invalid match on worker type check."),
    };
    add_tracker(tracker, worker.clone())?;

    Ok(())
}

/// Executes the workers in sequence.
async fn execute_one_shot(
    tracker: Vec<Worker>,
    handles: JoinSet<Handle>,
    database_arc: Arc<dyn Database>,
    sender: Sender<CoreMessage>,
    receiver: Receiver<CoreMessage>,
    token: String,
    targets: Vec<Target>,
) -> Result<()> {
    // Declare/take ownership of variables
    let mut tracker = tracker;
    let mut handles = handles;
    let discord_http;

    // Start Discord bot
    start_discord_bot(
        &mut tracker,
        &mut handles,
        database_arc.clone(),
        sender.clone(),
        token.clone(),
    )?;

    loop {
        // Await for Discord API
        tokio::select! {
            message = async { receiver.try_recv() } => {
                if message.is_err() {
                    continue;
                }
                let message = message.unwrap();
                match message {
                    CoreMessage::TransferDiscordHttp(api) => {
                        discord_http = Some(api);
                        break;
                    }
                _ => continue,
                }
            }

            Some(finished_handle) = handles.join_next() => {
                // Bail on JoinError
                if let Err(join_error) = finished_handle {
                    bail!("JoinError: {}", join_error);
                }

                // Bail since Discord Bot is dead anyway
                let finished_handle = finished_handle.unwrap();
                match finished_handle.1 {
                    Ok(_) => bail!("Discord thread exited"),
                    Err(error) => bail!(error),
                }
            }
        }
    }

    #[macro_export]
    macro_rules! await_handle {
        ($($arg: tt)*) => {
            while let Some(finished_handle) = handles.join_next().await {
                // Continue on JoinError
                if let Err(join_error) = finished_handle {
                    log!("{} JoinError: {}.", "[CORE]".blue(), join_error);
                    bail!(join_error);
                }
                let finished_handle = finished_handle.unwrap();

                // If worker is the same, break
                let worker = finished_handle.0;
                if worker == $($arg)* {
                    break;
                }

                // If DiscordBot thread ended, bail
                if worker == Worker::DiscordBot {
                    bail!("Discord Bot process ended.");
                }
            }
        };
    }

    // Start Gofer
    start_gofer(
        &mut tracker,
        &mut handles,
        database_arc.clone(),
        targets.clone(),
    )?;
    await_handle!(Worker::Gofer);

    // Start Announcer
    start_announcer(
        &mut tracker,
        &mut handles,
        database_arc.clone(),
        discord_http.clone(),
        None,
    )?;
    await_handle!(Worker::Announcer);

    // Disconnect Discord
    disconnect_discord(discord_http.as_ref().unwrap()).await?;

    log!(
        "{} One-shot execution finished. Terminating.",
        "[CORE]".blue()
    );

    Ok(())
}

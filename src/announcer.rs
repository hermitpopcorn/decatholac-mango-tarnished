use std::sync::Arc;

use anyhow::{anyhow, Result};
use chrono::Utc;
use colored::Colorize;
use serenity::http::Http;
use tokio::task::JoinSet;

use crate::{
    database::database::Database,
    discord::{get_channel_id, send_chapters},
    log,
    structs::Server,
    Worker,
};

/// Spawns one thread for each registered Server,
/// sending information on chapters that haven't been announced on that Server.
pub async fn dispatch_announcer(
    database: Arc<dyn Database>,
    discord_http: Arc<Http>,
) -> (Worker, Result<()>) {
    log!("{} Dispatching Announcer...", "[ANNO]".red());

    let servers = database.get_servers().await;
    if let Err(error) = servers {
        log!("{} Announcer could not fetch servers list.", "[ANNO]".red());
        return (Worker::Announcer, Err(anyhow!(error)));
    }
    let servers = servers.unwrap();

    let mut handles = JoinSet::new();

    for server in servers {
        let cloned_db = database.clone();
        let cloned_discord_http = discord_http.clone();
        handles.spawn(announce_for_server(cloned_db, cloned_discord_http, server));
    }

    while let Some(_) = handles.join_next().await {
        // Loop until all handles have finished
    }

    log!("{} Announcer has finished.", "[ANNO]".red());
    (Worker::Announcer, Ok(()))
}

pub async fn dispatch_solo_announcer(
    database: Arc<dyn Database>,
    discord_http: Arc<Http>,
    server: Server,
) -> (Worker, Result<()>) {
    log!(
        "{} Dispatching Solo Announcer for {}...",
        "[ANNO]".red(),
        server.identifier
    );

    let announce = announce_for_server(database, discord_http, server.clone()).await;

    match announce {
        Ok(_) => log!(
            "{} Solo Announcer for {} has finished.",
            "[ANNO]".red(),
            server.identifier,
        ),
        Err(error) => {
            log!(
                "{} Solo Announcer for {} failed: {}.",
                "[ANNO]".red(),
                server.identifier,
                error,
            );
            return (Worker::SoloAnnouncer(server), Err(anyhow!(error)));
        }
    }

    (Worker::SoloAnnouncer(server), Ok(()))
}

/// Child process of `dispatch_announcer`.
/// This function gets run for every thread.
async fn announce_for_server(
    database: Arc<dyn Database>,
    discord_http: Arc<Http>,
    server: Server,
) -> Result<()> {
    let is_announcing = database
        .get_announcing_server_flag(&server.identifier)
        .await?;
    if is_announcing {
        log!(
            "{} Skipping Server {} to prevent announcement conflicts.",
            "[ANNO]".red(),
            &server.identifier,
        );
        return Ok(());
    }
    database
        .set_announcing_server_flag(&server.identifier, true)
        .await?;

    let chapters = database
        .get_unnanounced_chapters(&server.identifier)
        .await?;
    if chapters.len() > 0 {
        log!(
            "{} Announcing {} chapters for Server {}...",
            "[ANNO]".red(),
            chapters.len(),
            &server.identifier
        );

        let channel = get_channel_id(&server.feed_channel_identifier)?;
        send_chapters(discord_http.as_ref(), channel, chapters).await?;
        database
            .set_last_announced_time(&server.identifier, &Utc::now())
            .await?;
    } else {
        log!(
            "{} No new chapters for Server {}.",
            "[ANNO]".red(),
            &server.identifier,
        );
    }

    database
        .set_announcing_server_flag(&server.identifier, false)
        .await?;

    Ok(())
}

use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use crossbeam::channel::Sender;
use serenity::http::Http;
use tokio::{spawn, sync::Mutex};

use crate::{
    database::database::Database,
    discord::{get_channel_id, send_chapters},
    log,
    structs::Server,
    CoreMessage,
};

pub async fn dispatch_announcer(
    database: Arc<Mutex<dyn Database>>,
    discord_http: Arc<Http>,
    sender: Sender<CoreMessage>,
) -> Result<()> {
    log!("Dispatching Announcer...");

    let servers = database.lock().await.get_servers()?;

    let mut handles = Vec::with_capacity(servers.len());

    for server in servers {
        let cloned_db = database.clone();
        let cloned_discord_http = discord_http.clone();
        let handle = spawn(announce_for_server(cloned_db, cloned_discord_http, server));
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.await?;
    }

    log!("Announcer has finished.");
    let _ = sender.send(CoreMessage::AnnouncerFinished)?;
    Ok(())
}

async fn announce_for_server(
    database: Arc<Mutex<dyn Database>>,
    discord_http: Arc<Http>,
    server: Server,
) -> Result<()> {
    let db_access = database.lock().await;
    let is_announcing = db_access.get_announcing_server_flag(&server.identifier)?;
    if is_announcing {
        return Ok(());
    }
    db_access.set_announcing_server_flag(&server.identifier, true)?;

    let chapters = db_access.get_unnanounced_chapters(&server.identifier)?;
    if chapters.len() > 0 {
        log!(
            "Announcing {} chapters for Server {}...",
            chapters.len(),
            &server.identifier
        );

        let channel = get_channel_id(&server.feed_channel_identifier)?;
        send_chapters(discord_http.as_ref(), channel, chapters).await?;
        db_access.set_last_announced_time(&server.identifier, &Utc::now())?;
    } else {
        log!("No new chapters for Server {}.", &server.identifier);
    }

    db_access.set_announcing_server_flag(&server.identifier, false)?;

    Ok(())
}
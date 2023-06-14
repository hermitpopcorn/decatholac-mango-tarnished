use std::sync::Arc;

use anyhow::Result;
use crossbeam::channel::Sender;
use serenity::http::Http;
use tokio::sync::Mutex;

use crate::{database::database::Database, log, CoreMessage};

pub async fn dispatch_announcer(
    database: Arc<Mutex<dyn Database>>,
    discord_http: Arc<Http>,
    sender: Sender<CoreMessage>,
) -> Result<()> {
    log!("Dispatching Announcer...");
    todo!()
}

use std::sync::Arc;

use anyhow::Result;
use config::{get_config, get_targets};
use database::sqlite::SqliteDatabase;
use gofer::dispatch_gofers;
use tokio::{spawn, sync::Mutex};

mod config;
mod database;
mod gofer;
mod parsers;
mod structs;
mod utils;

#[tokio::main]
async fn main() -> Result<()> {
    let config = get_config(Some("settings.toml"))?;
    let targets = get_targets(config.get("targets"))?;

    let database = SqliteDatabase::new("database.db");
    let database_arc = Arc::new(Mutex::new(database));

    let handler = spawn(dispatch_gofers(database_arc.clone(), targets.clone()));
    let _ = handler.await?;

    Ok(())
}

use anyhow::Result;
use config::{get_config, get_targets};
use database::sqlite::SqliteDatabase;
use gofer::fetch_body;

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
    let test = fetch_body(&targets[0].source, &targets[0].request_headers).await?;
    println!("{}", test);

    let database = SqliteDatabase::new("database.db");

    Ok(())
}

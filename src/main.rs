use anyhow::Result;
use config::{get_config, get_targets};
use database::{database::Database, sqlite::SqliteDatabase};
use gofer::fetch_body;
use parsers::rss::parse_rss;

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
    let test = fetch_body(&targets[1].source, &targets[1].request_headers).await?;
    let chapters = parse_rss(&targets[1], &test);

    let database = SqliteDatabase::new("database.db");
    database.save_chapters(&chapters)?;

    Ok(())
}

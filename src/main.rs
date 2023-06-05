use anyhow::Result;
use config::{get_config, get_targets};
use database::sqlite::SqliteDatabase;

mod config;
mod database;
mod parsers;
mod structs;
mod utils;

fn main() -> Result<()> {
    let config = get_config(Some("settings.toml"))?;
    let targets = get_targets(config.get("targets"))?;
    println!("{:#?}", targets.len());
    println!("{:#?}", targets);

    let database = SqliteDatabase::new("database.db");

    Ok(())
}

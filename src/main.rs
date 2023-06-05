use anyhow::Result;
use config::{get_config, get_targets};

mod config;
mod parsers;
mod structs;

fn main() -> Result<()> {
    let config = get_config(Some("config.toml"))?;
    let targets = get_targets(config.get("targets"))?;
    println!("{:#?}", targets.len());
    println!("{:#?}", targets);

    Ok(())
}

use std::sync::Arc;

use anyhow::{anyhow, bail, Result};
use crossbeam::channel::Sender;
use poise::{serenity_prelude as serenity, Framework, FrameworkBuilder};
use tokio::sync::Mutex;
use toml::Value as TomlValue;

use crate::{database::database::Database, log, CoreMessage};
struct Data {
    sender: Sender<CoreMessage>,
}
type PoiseError = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, PoiseError>;

pub fn get_discord_token(token: Option<&TomlValue>) -> Result<String> {
    if token.is_none() {
        bail!("Discord token not found.")
    }

    let token = token
        .unwrap()
        .as_str()
        .ok_or(anyhow!("Discord token is not a string."))?;

    let token = token.to_owned();
    Ok(token)
}

pub async fn connect_discord(
    database: Arc<Mutex<dyn Database>>,
    sender: Sender<CoreMessage>,
    token: String,
) -> Result<()> {
    log!("Connecting to Discord...");

    let framework: FrameworkBuilder<Data, PoiseError> = Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![trigger_start_gofer()],
            ..Default::default()
        })
        .token(token)
        .intents(serenity::GatewayIntents::non_privileged())
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                log!("Connected to Discord.");
                Ok(Data { sender: sender })
            })
        });

    framework.run().await?;

    Ok(())
}

/// Manually trigger the fetch process for new chapters.
#[poise::command(slash_command, ephemeral, rename = "fetch")]
async fn trigger_start_gofer(ctx: Context<'_>) -> Result<(), PoiseError> {
    let _ = ctx.data().sender.send(CoreMessage::StartGofer)?;
    ctx.say("Fetching process triggered.").await?;
    Ok(())
}

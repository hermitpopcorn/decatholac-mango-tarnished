use std::sync::Arc;

use anyhow::Result;
use crossbeam::channel::Sender;
use poise::{
    serenity_prelude::{self as serenity, ChannelId, Http},
    Framework, FrameworkBuilder,
};
use tokio::sync::Mutex;

use crate::{database::database::Database, log, structs::Chapter, CoreMessage};
struct Data {
    sender: Sender<CoreMessage>,
    database: Arc<Mutex<dyn Database>>,
}
type PoiseError = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, PoiseError>;

pub async fn connect_discord(
    database: Arc<Mutex<dyn Database>>,
    sender: Sender<CoreMessage>,
    token: String,
) -> Result<()> {
    log!("[DSCD] Connecting to Discord...");

    let framework: FrameworkBuilder<Data, PoiseError> = Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                trigger_start_gofer(),
                trigger_start_announcer(),
                set_as_feed_channel(),
            ],
            ..Default::default()
        })
        .token(token)
        .intents(serenity::GatewayIntents::non_privileged())
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                log!("[DSCD] Connected to Discord.");

                // Send Discord API back to core control
                log!("[DSCD] Sending Discord API back to core control...");
                let discord_http = ctx.http.clone();
                sender.send(CoreMessage::TransferDiscordHttp(discord_http))?;

                Ok(Data {
                    sender: sender,
                    database: database,
                })
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

/// Print all unannounced feed items.
#[poise::command(slash_command, ephemeral, rename = "announce")]
async fn trigger_start_announcer(ctx: Context<'_>) -> Result<(), PoiseError> {
    let _ = ctx.data().sender.send(CoreMessage::StartAnnouncer)?;
    ctx.say("Announcement process triggered.").await?;
    Ok(())
}

/// Set current channel as the feed channel. You must have channel management permissions to do this.
#[poise::command(
    slash_command,
    ephemeral,
    rename = "set-as-feed-channel",
    default_member_permissions = "MANAGE_CHANNELS"
)]
async fn set_as_feed_channel(ctx: Context<'_>) -> Result<(), PoiseError> {
    let guild_id = ctx.guild_id();
    if guild_id.is_none() {
        ctx.say("Could not get Server ID.").await?;
        return Ok(());
    }
    let guild_id = guild_id.unwrap();
    let channel_id = ctx.channel_id();

    let db = ctx.data().database.lock().await;
    db.set_feed_channel(
        guild_id.to_string().as_str(),
        channel_id.to_string().as_str(),
    )?;

    ctx.say("This channel has been set as the feed channel.")
        .await?;
    Ok(())
}

pub fn get_channel_id(channel_id: &str) -> Result<ChannelId> {
    Ok(ChannelId(channel_id.parse()?))
}

pub async fn send_chapters(http: &Http, channel: ChannelId, chapters: Vec<Chapter>) -> Result<()> {
    for chapter in chapters {
        let title = format!("[{}] {}", chapter.manga, chapter.title);
        channel
            .send_message(http, |m| {
                m.embed(|e| e.timestamp(chapter.date).title(title).url(chapter.url))
            })
            .await?;
    }

    Ok(())
}

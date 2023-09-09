use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::structs::{Chapter, Server};

/// This trait works as an "interface" to Database implementations.
#[async_trait]
pub trait Database: Send + Sync {
    /// Initialize the database by creating tables and setting up indexes if they don't exist already.
    /// This function should be called by the `new` function if it determines that the database needs setup.
    async fn initialize_database(&self) -> Result<()>;

    /// Saves a vector of Chapters into the database.
    async fn save_chapters(&self, chapters: &[Chapter]) -> Result<()>;
    /// Fetches a vector of chapters that have not been announced for a certain Server.
    async fn get_unnanounced_chapters(&self, guild_id: &str) -> Result<Vec<Chapter>>;

    /// Gets a single server by their ID from the database.
    async fn get_server(&self, guild_id: &str) -> Result<Server>;
    /// Fetches the entire list of Servers that are in the database.
    async fn get_servers(&self) -> Result<Vec<Server>>;
    /// Fetches the Channel ID of the text channel that's set as the "feed channel" for a certain Server.
    async fn get_feed_channel(&self, guild_id: &str) -> Result<String>;
    /// Sets the "feed channel" for a certain Server.
    async fn set_feed_channel(&self, guild_id: &str, channel_id: &str) -> Result<()>;

    /// Fetches the last announced time for a certain Server.
    /// This function should be used to determine unnanounced chapters by `get_unnanounced_chapters`.
    async fn get_last_announced_time(&self, guild_id: &str) -> Result<DateTime<Utc>>;
    /// Sets the last announced time for a certain Server.
    async fn set_last_announced_time(
        &self,
        guild_id: &str,
        last_announced_at: &DateTime<Utc>,
    ) -> Result<()>;

    /// Fetches the "announcing" flag for a certain Server.
    /// If the value is truthy, then the Announcer should not process another announcement on that Server.
    async fn get_announcing_server_flag(&self, guild_id: &str) -> Result<bool>;
    /// Sets the "announcing" flag for a certain Server.
    /// Having it set to truthy should block the Announcer
    /// from running a second announcement while another one is in progress.
    async fn set_announcing_server_flag(&self, guild_id: &str, announcing: bool) -> Result<()>;
}

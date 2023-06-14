use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::structs::{Chapter, Server};

pub trait Database: Send {
    fn initialize_database(&self) -> Result<()>;

    fn save_chapters(&self, chapters: &[Chapter]) -> Result<()>;
    fn get_unnanounced_chapters(&self, guild_id: &str) -> Result<Vec<Chapter>>;

    fn get_servers(&self) -> Result<Vec<Server>>;
    fn get_feed_channel(&self, guild_id: &str) -> Result<String>;
    fn set_feed_channel(&self, guild_id: &str, channel_id: &str) -> Result<()>;

    fn get_last_announced_time(&self, guild_id: &str) -> Result<DateTime<Utc>>;
    fn set_last_announced_time(
        &self,
        guild_id: &str,
        last_announced_at: &DateTime<Utc>,
    ) -> Result<()>;

    fn get_announcing_server_flag(&self, guild_id: &str) -> Result<bool>;
    fn set_announcing_server_flag(&self, guild_id: &str, announcing: bool) -> Result<()>;
}

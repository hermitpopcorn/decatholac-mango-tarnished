use std::collections::HashMap;

use chrono::prelude::*;
use serde_json::Value;

/// Contains information of a Server that's registered to the bot.
#[derive(Debug, Clone)]
pub struct Server {
    pub identifier: String,
    pub feed_channel_identifier: String,
}

impl PartialEq for Server {
    fn eq(&self, other: &Self) -> bool {
        self.identifier == other.identifier
    }
}

/// Chapter of a manga.
/// `number` may not be the same as the chapter's release number,
/// just an unique identifier to set it apart from other chapters.
#[derive(Debug, Clone)]
pub struct Chapter {
    pub manga: String,
    pub number: String,
    pub title: String,
    pub date: DateTime<Utc>,
    pub url: String,
    pub logged_at: Option<DateTime<Utc>>,
    pub announced_at: DateTime<Utc>,
}

/// The three supported parse modes.
#[derive(Debug, Clone)]
pub enum ParseMode {
    Rss,
    Json,
    Html,
}

/// Each target defines a source to get manga updates from.
#[derive(Debug, Clone)]
pub struct Target {
    pub name: String,
    pub source: String,
    /// Whether the source lists item A->Z (old chapters first) instead of Z->A (new chapters first).
    pub ascending_source: bool,
    pub mode: ParseMode,
    pub base_url: Option<String>,
    pub request_headers: Option<HashMap<String, String>>,
    /// How much time to delay the announcement of new chapters (in days).
    pub delay: Option<u8>,
    // JSON mode
    pub keys: Option<TargetKeys>,
    // HTML mode
    pub tags: Option<TargetTags>,
}

/// Enum of supported datetime parse formats for the JSON parser.
#[derive(Debug, Clone)]
pub enum JsonDateTimeFormat {
    UnixSec,
    UnixMilli,
    UnixNano,
    Rfc2822,
    Rfc3339,
    StringFormat(String),
}

/// JSON object keys information for parsing from a JSON source.
#[derive(Debug, Clone)]
pub struct TargetKeys {
    pub chapters: String,
    pub number: Vec<String>,
    pub title: Vec<String>,
    pub date: String,
    pub date_format: Option<JsonDateTimeFormat>,
    pub url: String,
    pub skip: HashMap<String, Value>,
}

/// Strings of tag and attribute names for parsing from a HTML source.
#[derive(Debug, Clone)]
pub struct TargetTags {
    pub chapters_tag: String,
    pub number_tag: Option<String>,
    pub number_attribute: Option<String>,
    pub title_tag: Option<String>,
    pub title_attribute: Option<String>,
    pub date_tag: Option<String>,
    pub date_attribute: Option<String>,
    /// How to parse the date text. Uses strftime format notation.
    pub date_format: Option<String>,
    pub url_tag: Option<String>,
    pub url_attribute: Option<String>,
}

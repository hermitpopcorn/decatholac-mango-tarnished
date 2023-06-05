use std::collections::HashMap;

use chrono::prelude::*;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct Server {
    pub identifier: String,
    pub feed_channel_identifier: String,
    pub last_announced_at: DateTime<Utc>,
    pub is_announcing: bool,
}

#[derive(Debug, Clone)]
pub struct Chapter {
    pub manga: String,
    pub number: String,
    pub title: String,
    pub date: DateTime<Utc>,
    pub url: String,
    pub logged_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub enum ParseMode {
    Rss,
    Json,
    Html,
}

#[derive(Debug, Clone)]
pub struct Target {
    pub name: String,
    pub source: String,
    pub ascending_source: bool, // Whether the source lists item A->Z instead of Z->A like normal
    pub mode: ParseMode,
    pub base_url: Option<String>,
    pub request_headers: Option<HashMap<String, String>>,
    // JSON mode
    pub keys: Option<TargetKeys>,
    // HTML mode
    pub tags: Option<TargetTags>,
}

#[derive(Debug, Clone)]
pub enum JsonDateTimeFormat {
    Unix,
    Rfc2822,
    Rfc3339,
}

#[derive(Debug, Clone)]
pub struct TargetKeys {
    pub chapters: String,
    pub number: String,
    pub title: String,
    pub date: String,
    pub date_format: Option<JsonDateTimeFormat>,
    pub url: String,
    pub skip: HashMap<String, Value>,
}

#[derive(Debug, Clone)]
pub struct TargetTags {
    pub chapters_tag: String,
    pub number_tag: Option<String>,
    pub number_attribute: Option<String>,
    pub title_tag: Option<String>,
    pub title_attribute: Option<String>,
    pub date_tag: Option<String>,
    pub date_attribute: Option<String>,
    pub date_format: Option<String>,
    pub url_tag: Option<String>,
    pub url_attribute: Option<String>,
}

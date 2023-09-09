use std::{sync::Arc, vec};

use anyhow::{bail, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use colored::Colorize;
use rusqlite::{params, Connection, OptionalExtension};
use tokio::sync::Mutex;

use crate::{
    log,
    structs::{Chapter, Server},
};

use super::database::Database;

pub struct SqliteDatabase {
    connection: Arc<Mutex<Connection>>,
}

impl SqliteDatabase {
    pub async fn new(path: &str) -> Self {
        let connection = Connection::open(path).unwrap();

        let new = Self {
            connection: Arc::new(Mutex::new(connection)),
        };
        new.initialize_database().await.unwrap();

        new
    }
}

#[async_trait]
impl Database for SqliteDatabase {
    async fn initialize_database(&self) -> Result<()> {
        let connection = self.connection.lock().await;

        let mut statement = connection
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'Chapters'")?;
        let check = statement.query_row([], |_row| Ok(())).optional()?;

        if check.is_none() {
            log!("{} Initializing Chapters table...", "[DATA]".yellow());
            connection.execute(
                "CREATE TABLE 'Chapters' (
                    'id'          INTEGER,
                    'manga'       VARCHAR(255) NOT NULL,
                    'title'       VARCHAR(255) NOT NULL,
                    'number'      VARCHAR(255) NOT NULL,
                    'url'         VARCHAR(255) NOT NULL,
                    'date'        DATETIME NOT NULL,
                    'loggedAt'    DATETIME NOT NULL,
                    'announcedAt' DATETIME NOT NULL,
                    PRIMARY KEY('id' AUTOINCREMENT)
                )",
                [],
            )?;
        }

        let mut statement = connection
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'Servers'")?;
        let check = statement.query_row([], |_row| Ok(())).optional()?;

        if check.is_none() {
            log!("{} Initializing Servers table...", "[DATA]".yellow());
            connection.execute(
                "CREATE TABLE 'Servers' (
                    'id'              INTEGER,
                    'guildId'         VARCHAR(255) NOT NULL,
                    'channelId'       VARCHAR(255),
                    'lastAnnouncedAt' DATETIME,
                    'isAnnouncing'    INTEGER DEFAULT 0,
                    PRIMARY KEY('id' AUTOINCREMENT)
                )",
                [],
            )?;
        }

        Ok(())
    }

    async fn save_chapters(&self, chapters: &[Chapter]) -> Result<()> {
        let connection = self.connection.lock().await;

        for chapter in chapters {
            let mut statement = connection.prepare(
                "SELECT id FROM Chapters WHERE manga = ?1 AND title = ?2 AND number = ?3",
            )?;
            let check = statement
                .query_row(
                    params![&chapter.manga, &chapter.title, &chapter.number],
                    |_row| Ok(()),
                )
                .optional()?;

            if check.is_some() {
                continue;
            }

            log!(
                "{} Saving new chapter... [{}]: {}{}",
                "[DATA]".yellow(),
                &chapter.manga,
                &chapter.title,
                (|| {
                    if &chapter.date == &chapter.announced_at {
                        return String::from("");
                    }

                    format!(
                        " (Will be announced on {})",
                        &chapter.announced_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                    )
                })(),
            );
            let mut statement = connection.prepare(
                "INSERT INTO Chapters
                (manga, title, number, url, date, loggedAt, announcedAt)
                VALUES
                (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            statement.execute(params![
                &chapter.manga,
                &chapter.title,
                &chapter.number,
                &chapter.url,
                &chapter.date,
                Utc::now(),
                &chapter.announced_at,
            ])?;
        }

        Ok(())
    }

    async fn get_unnanounced_chapters(&self, guild_id: &str) -> Result<Vec<Chapter>> {
        let last_announced_at = match self.get_last_announced_time(guild_id).await {
            Ok(time) => time,
            Err(_) => bail!("Could not get last announced time for the Server."),
        };

        let connection = self.connection.lock().await;

        let mut chapters = vec![];

        let mut statement = connection.prepare(
            "SELECT manga, title, number, url, date, loggedAt, announcedAt
            FROM Chapters
            WHERE announcedAt > ?1 AND ?2 >= announcedAt
            ORDER BY date ASC",
        )?;
        let mut result = statement.query(params![last_announced_at, Utc::now()])?;
        while let Some(row) = result.next()? {
            chapters.push(Chapter {
                manga: row.get(0)?,
                title: row.get(1)?,
                number: row.get(2)?,
                url: row.get(3)?,
                date: row.get(4)?,
                logged_at: row.get(5)?,
                announced_at: row.get(6)?,
            });
        }

        Ok(chapters)
    }

    async fn get_server(&self, guild_id: &str) -> Result<Server> {
        let channel_id = self.get_feed_channel(guild_id).await;

        if channel_id.is_err() {
            bail!("Feed channel has not been set for this server.")
        }

        Ok(Server {
            identifier: String::from(guild_id),
            feed_channel_identifier: channel_id?,
        })
    }

    async fn get_servers(&self) -> Result<Vec<Server>> {
        let connection = self.connection.lock().await;
        let mut statement = connection.prepare("SELECT guildId, channelId FROM Servers")?;
        let mut result = statement.query([])?;

        let mut servers = vec![];
        while let Some(row) = result.next()? {
            servers.push(Server {
                identifier: row.get(0)?,
                feed_channel_identifier: row.get(1)?,
            });
        }

        Ok(servers)
    }

    async fn set_feed_channel(&self, guild_id: &str, channel_id: &str) -> Result<()> {
        log!(
            "{} Setting new feed channel for Server {}...",
            "[DATA]".yellow(),
            &guild_id
        );

        let currently_set_channel_id = self.get_feed_channel(guild_id).await;

        if currently_set_channel_id
            .as_ref()
            .is_ok_and(|id| id.eq(&channel_id))
        {
            return Ok(());
        }

        let connection = self.connection.lock().await;

        match currently_set_channel_id {
            Ok(_) => {
                let mut statement =
                    connection.prepare("UPDATE Servers SET channelId = ?2 WHERE guildId = ?1")?;
                statement.execute(params![guild_id, channel_id])?;
            }
            Err(_) => {
                let mut statement = connection.prepare(
                    "INSERT INTO Servers (guildId, channelId, lastAnnouncedAt) VALUES (?1, ?2, ?3)",
                )?;
                statement.execute(params![guild_id, channel_id, Utc::now()])?;
            }
        }

        Ok(())
    }

    async fn get_feed_channel(&self, guild_id: &str) -> Result<String> {
        let connection = self.connection.lock().await;
        let mut statement =
            connection.prepare("SELECT channelId FROM Servers WHERE guildId = ?1")?;
        let check = statement.query_row(params![guild_id], |row| {
            let row_channel_id: String = row.get(0)?;
            Ok(row_channel_id)
        });

        match check {
            Ok(channel_id) => Ok(channel_id),
            Err(_) => bail!("Feed channel has not been set for this server."),
        }
    }

    async fn get_last_announced_time(&self, guild_id: &str) -> Result<DateTime<Utc>> {
        let connection = self.connection.lock().await;
        let mut statement =
            connection.prepare("SELECT lastAnnouncedAt FROM Servers WHERE guildId = ?1")?;
        let last_announced_at = statement.query_row(params![&guild_id], |row| {
            let row_last_announced_at: DateTime<Utc> = row.get(0)?;
            Ok(row_last_announced_at)
        });

        if last_announced_at.is_err() {
            bail!("Feed channel has not been set for this server.");
        }

        Ok(last_announced_at?)
    }

    async fn set_last_announced_time(
        &self,
        guild_id: &str,
        last_announced_at: &DateTime<Utc>,
    ) -> Result<()> {
        let connection = self.connection.lock().await;
        let mut statement =
            connection.prepare("UPDATE Servers SET lastAnnouncedAt = ?1 WHERE guildId = ?2")?;
        let result = statement.execute(params![last_announced_at, guild_id])?;

        if result < 1 {
            bail!("Feed channel has not been set for this server.");
        }

        Ok(())
    }

    async fn get_announcing_server_flag(&self, guild_id: &str) -> Result<bool> {
        let connection = self.connection.lock().await;
        let mut statement =
            connection.prepare("SELECT isAnnouncing FROM Servers WHERE guildId = :g")?;
        let check = statement.query_row(&[(":g", guild_id)], |row| {
            let row_channel_id: bool = row.get(0)?;
            Ok(row_channel_id)
        });

        match check {
            Ok(is_announcing) => return Ok(is_announcing),
            Err(_) => bail!("Feed channel has not been set for this server."),
        };
    }

    async fn set_announcing_server_flag(&self, guild_id: &str, announcing: bool) -> Result<()> {
        let connection = self.connection.lock().await;
        let mut statement =
            connection.prepare("UPDATE Servers SET isAnnouncing = :s WHERE guildId = :g")?;
        let result = statement.execute(&[
            (":g", guild_id),
            (
                ":s",
                match announcing {
                    true => "1",
                    false => "0",
                },
            ),
        ])?;

        if result < 1 {
            bail!("Feed channel has not been set for this server.");
        }

        Ok(())
    }
}

use std::vec;

use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    log,
    structs::{Chapter, Server},
};

use super::database::Database;

pub struct SqliteDatabase {
    connection: Connection,
}

impl SqliteDatabase {
    pub fn new(path: &str) -> Self {
        let connection = Connection::open(path).unwrap();

        let new = Self {
            connection: connection,
        };
        new.initialize_database().unwrap();

        new
    }
}

impl Database for SqliteDatabase {
    fn initialize_database(&self) -> Result<()> {
        let mut statement = self
            .connection
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'Chapters'")?;
        let check = statement.query_row([], |_row| Ok(())).optional()?;

        if check.is_none() {
            log!("Initializing Chapters table...");
            self.connection.execute(
                "CREATE TABLE 'Chapters' (
                    'id'       INTEGER,
                    'manga'    VARCHAR(255) NOT NULL,
                    'title'    VARCHAR(255) NOT NULL,
                    'number'   VARCHAR(255) NOT NULL,
                    'url'      VARCHAR(255) NOT NULL,
                    'date'     DATETIME,
                    'loggedAt' DATETIME NOT NULL,
                    PRIMARY KEY('id' AUTOINCREMENT)
                )",
                [],
            )?;
        }

        let mut statement = self
            .connection
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'Servers'")?;
        let check = statement.query_row([], |_row| Ok(())).optional()?;

        if check.is_none() {
            log!("Initializing Servers table...");
            self.connection.execute(
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

        let mut statement = self.connection.prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'Subscriptions'",
        )?;
        let check = statement.query_row([], |_row| Ok(())).optional()?;

        if check.is_none() {
            log!("Initializing Subscriptions table...");
            self.connection.execute(
                "CREATE TABLE 'Subscriptions' (
                    'id'      INTEGER,
                    'guildId' VARCHAR(255) NOT NULL,
                    'userId'  VARCHAR(255) NOT NULL,
                    'title'   VARCHAR(255) NOT NULL,
                    PRIMARY KEY('id' AUTOINCREMENT)
                )",
                [],
            )?;
        }

        Ok(())
    }

    fn save_chapters(&self, chapters: &[Chapter]) -> Result<()> {
        for chapter in chapters {
            let mut statement = self.connection.prepare(
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
                "Saving new chapter... [{}]: {}",
                &chapter.manga,
                &chapter.title
            );
            let mut statement = self.connection.prepare(
                "INSERT INTO Chapters
                (manga, title, number, url, date, loggedAt)
                VALUES
                (:manga, :title, :number, :url, :date, :logged_at)",
            )?;
            statement.execute(params![
                &chapter.manga,
                &chapter.title,
                &chapter.number,
                &chapter.url,
                &chapter.date,
                Utc::now(),
            ])?;
        }

        Ok(())
    }

    fn get_unnanounced_chapters(&self, guild_id: &str) -> Result<Vec<Chapter>> {
        let last_announced_at = match self.get_last_announced_time(guild_id) {
            Ok(time) => time,
            Err(_) => bail!("Could not get last announced time for the Server."),
        };

        let mut chapters = vec![];

        let mut statement = self.connection.prepare(
            "SELECT manga, title, number, url, date, loggedAt
            FROM Chapters
            WHERE loggedAt > ?1
            AND date > ?1
            ORDER BY date ASC",
        )?;
        let mut result = statement.query(params![last_announced_at])?;
        while let Some(row) = result.next()? {
            chapters.push(Chapter {
                manga: row.get(0)?,
                title: row.get(1)?,
                number: row.get(2)?,
                url: row.get(3)?,
                date: row.get(4)?,
                logged_at: row.get(5)?,
            });
        }

        Ok(chapters)
    }

    fn get_servers(&self) -> Result<Vec<crate::structs::Server>> {
        let mut statement = self
            .connection
            .prepare("SELECT guildId, channelId, lastAnnouncedAt, isAnnouncing FROM Servers")?;
        let mut result = statement.query([])?;

        let mut servers = vec![];
        while let Some(row) = result.next()? {
            let is_announcing: i8 = row.get(3)?;
            let is_announcing = match is_announcing {
                1 => true,
                0 => false,
                _ => bail!("Invalid bool value"),
            };

            servers.push(Server {
                identifier: row.get(0)?,
                feed_channel_identifier: row.get(1)?,
                last_announced_at: row.get(2)?,
                is_announcing: is_announcing,
            });
        }

        Ok(servers)
    }

    fn set_feed_channel(&self, guild_id: &str, channel_id: &str) -> Result<()> {
        let currently_set_channel_id = self.get_feed_channel(guild_id);

        if currently_set_channel_id
            .as_ref()
            .is_ok_and(|id| id.eq(&channel_id))
        {
            return Ok(());
        }

        let mut statement = self.connection.prepare(match currently_set_channel_id {
            Ok(_) => "UPDATE Servers SET channelId = ?1 WHERE guildId = ?2",
            Err(_) => {
                "INSERT INTO Servers (guildId, channelId, lastAnnouncedAt) VALUES (?2, ?1, ?3)"
            }
        })?;
        statement.execute(params![guild_id, channel_id, Utc::now()])?;

        Ok(())
    }

    fn get_feed_channel(&self, guild_id: &str) -> Result<String> {
        let mut statement = self
            .connection
            .prepare("SELECT channelId FROM Servers WHERE guildId = ?1")?;
        let check = statement.query_row(params![guild_id], |row| {
            let row_channel_id: String = row.get(0)?;
            Ok(row_channel_id)
        });

        match check {
            Ok(channel_id) => Ok(channel_id),
            Err(_) => bail!("Feed channel has not been set for this server."),
        }
    }

    fn get_last_announced_time(&self, guild_id: &str) -> Result<DateTime<Utc>> {
        let mut statement = self
            .connection
            .prepare("SELECT lastAnnouncedAt FROM Servers WHERE guildId = ?1")?;
        let last_announced_at = statement.query_row(params![&guild_id], |row| {
            let row_last_announced_at: DateTime<Utc> = row.get(0)?;
            Ok(row_last_announced_at)
        });

        if last_announced_at.is_err() {
            bail!("Feed channel has not been set for this server.");
        }

        Ok(last_announced_at?)
    }

    fn set_last_announced_time(
        &self,
        guild_id: &str,
        last_announced_at: &DateTime<Utc>,
    ) -> Result<()> {
        let mut statement = self
            .connection
            .prepare("UPDATE Servers SET lastAnnouncedAt = ?1 WHERE guildId = ?2")?;
        let result = statement.execute(params![last_announced_at, guild_id])?;

        if result < 1 {
            bail!("Feed channel has not been set for this server.");
        }

        Ok(())
    }

    fn get_announcing_server_flag(&self, guild_id: &str) -> Result<bool> {
        let mut statement = self
            .connection
            .prepare("SELECT isAnnouncing FROM Servers WHERE guildId = :g")?;
        let check = statement.query_row(&[(":g", guild_id)], |row| {
            let row_channel_id: bool = row.get(0)?;
            Ok(row_channel_id)
        });

        let is_announcing = match check {
            Ok(is_announcing) => return Ok(is_announcing),
            Err(_) => bail!("Feed channel has not been set for this server."),
        };
    }

    fn set_announcing_server_flag(&self, guild_id: &str, announcing: bool) -> Result<()> {
        let mut statement = self
            .connection
            .prepare("UPDATE Servers SET isAnnouncing = :s WHERE guildId = :g")?;
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

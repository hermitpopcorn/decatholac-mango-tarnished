use anyhow::Result;
use rusqlite::{Connection, OptionalExtension};

use crate::log;

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
}

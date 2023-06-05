use anyhow::Result;

use crate::structs::Chapter;

pub trait Database: Send {
    fn initialize_database(&self) -> Result<()>;
    fn save_chapters(&self, chapters: &[Chapter]) -> Result<()>;
}

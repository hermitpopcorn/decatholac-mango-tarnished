use anyhow::Result;

pub trait Database {
    fn initialize_database(&self) -> Result<()>;
}

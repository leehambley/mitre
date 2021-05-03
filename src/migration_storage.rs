use super::{Error, Migration, MigrationList};

pub trait MigrationStorage: MigrationList {
    #[cfg(test)]
    fn reset(&mut self) -> Result<(), Error>;

    fn add(&mut self, _: Migration) -> Result<(), Error>;
    fn remove(&mut self, _: Migration) -> Result<(), Error>;
}

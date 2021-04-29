use super::{Error, Migration, MigrationList};

pub trait MigrationStorage: MigrationList {
    fn add(&mut self, _: Migration) -> Result<(), Error>;
    fn remove(&mut self, _: Migration) -> Result<(), Error>;
}

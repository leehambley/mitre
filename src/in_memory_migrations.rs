use super::{Error, Migration, MigrationList, MigrationStorage};
use std::vec::IntoIter;

pub struct InMemoryMigrations {
    m: Vec<Migration>,
}

impl MigrationList for InMemoryMigrations {
    fn all(&mut self) -> Result<IntoIter<Migration>, Error> {
        Ok(self.m.clone().into_iter())
    }
}

impl MigrationStorage for InMemoryMigrations {
    #[cfg(test)]
    fn reset(&mut self) -> Result<(), Error> {
        self.m = vec![];
        Ok(())
    }
    fn add(&mut self, _: Migration) -> Result<(), Error> {
        Ok(())
    }
    fn remove(&mut self, _: Migration) -> Result<(), Error> {
        Ok(())
    }
}

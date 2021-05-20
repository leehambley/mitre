use super::{Error, Migration, MigrationList, MigrationStorage};

pub struct InMemoryMigrations {
    pub m: Vec<Migration>,
}

impl InMemoryMigrations {
    pub fn new() -> Self {
        InMemoryMigrations { m: vec![] }
    }
}

impl Default for InMemoryMigrations {
    fn default() -> Self {
        Self::new()
    }
}

impl MigrationList for InMemoryMigrations {
    type Iterator = std::vec::IntoIter<Migration>;
    fn all(&mut self) -> Result<Self::Iterator, Error> {
        Ok(self.m.clone().into_iter())
    }
}

impl MigrationStorage for InMemoryMigrations {
    #[cfg(test)]
    fn reset(&mut self) -> Result<(), Error> {
        self.m = vec![];
        Ok(())
    }
    fn add(&mut self, m: Migration) -> Result<(), Error> {
        self.m.push(m);
        Ok(())
    }
    fn remove(&mut self, m: Migration) -> Result<(), Error> {
        let index = self.m.iter().position(|x| *x == m).unwrap();
        self.m.remove(index);
        Ok(())
    }
}

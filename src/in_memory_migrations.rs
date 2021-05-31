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
    type Item = Migration;
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn all(&mut self) -> Result<Box<(dyn Iterator<Item = Migration> + 'static)>, Error> {
        Ok(Box::new(self.m.clone().into_iter()))
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

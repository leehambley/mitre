use super::{Error, Migration};
pub use std::vec::IntoIter;

pub mod from_disk;
pub use from_disk::from_disk; // TODO: necessary?

// Prefer having a MigrationList trait, as here all() can return a Result<T, E> where
// with a pure Iterator<Item=Migration> we would lose that ability (and maybe have
// to use a Iterator<Item=<Result<Migration, E>> or something)
pub trait MigrationList {
    fn all<'a>(&'a mut self) -> Result<Box<(dyn Iterator<Item = Migration> + 'a)>, Error>;
}

impl MigrationList for &mut Box<dyn MigrationList> {
    fn all<'a>(&'a mut self) -> Result<Box<(dyn Iterator<Item = Migration> + 'a)>, Error> {
        (**self).all()
    }
}

#[cfg(test)]
mod tests {
    // Most all the bahaviour makes sense when tested
    // as part of migration storage, which implies migration list
    // so there's not much to see here, except placeholder code.
}

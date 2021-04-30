use super::{Error, Migration};
pub use std::vec::IntoIter;

pub mod from_disk;
pub use from_disk::from_disk;

pub trait MigrationList {
    fn all(&mut self) -> Result<IntoIter<Migration>, Error>;
}

#[cfg(test)]
mod tests {

    // Most all the bahaviour makes sense when tested
    // as part of migration storage, which implies migration list
    // so there's not much to see here, except placeholder code.
}

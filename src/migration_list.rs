use super::{Error, Migration};
pub use std::vec::IntoIter;

pub mod from_disk;
pub use from_disk::from_disk;

// TODO: this type is redundant now we know how to reference
// a `impl Iterator<T>` from a trait, we can can _simply_ use
// that.
pub trait MigrationList {
    type Item;
    type IntoIter: Iterator<Item = Self::Item>;
    fn all(&mut self) -> Result<Box<dyn Iterator<Item = Self::Item>>, Error>;
}

#[cfg(test)]
mod tests {

    // Most all the bahaviour makes sense when tested
    // as part of migration storage, which implies migration list
    // so there's not much to see here, except placeholder code.
}

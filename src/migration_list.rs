use super::{Error, Migration};
use std::vec::IntoIter;

pub trait MigrationList {
    fn all(&self) -> Result<IntoIter<Migration>, Error>;
}

#[cfg(test)]
mod tests {

    // Most all the bahaviour makes sense when tested
    // as part of migration storage, which implies migration list
    // so there's not much to see here, except placeholder code.
}

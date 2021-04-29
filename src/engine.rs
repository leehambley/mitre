use super::{Error, MigrationList, MigrationStateTuple, MigrationStorage};
use std::vec::IntoIter;
pub trait Engine {
    // formerly state store
    fn diff(
        src: impl MigrationList,
        dest: impl MigrationStorage,
    ) -> Result<IntoIter<MigrationStateTuple>, Error>;

    // TODO: dry-run?

    fn apply(
        // TODO: This should maybe get a _filtered_ list, or some query plan?
        src: impl MigrationList,
        dest: impl MigrationStorage,
    ) -> Result<IntoIter<MigrationStateTuple>, Error>;
}

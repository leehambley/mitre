use super::{Error, Migration};

pub enum DriverResult {
    // Sucessfully applied the migration
    Success,
    // There's nothing to do at all
    NothingToDo,
    // This driver does not know how to run migrations of type X
    MigrationRunnerMismatch,
}

// A Driver that has no work to do must report NothingToDo (e.g trying
// to unapply an irreversible migration, because this is a successful
// no-op)
pub trait Driver {
    // Apply will take a Migration and run any
    fn apply(&mut self, _: &Migration) -> Result<DriverResult, Error>;
    fn unapply(&mut self, _: &Migration) -> Result<DriverResult, Error>;
}

// Test that all drivers raise malformed migration when the migration
// has up, and change parts.

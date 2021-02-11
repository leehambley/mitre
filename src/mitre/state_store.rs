use super::migrations::Migration;

pub trait MigrationStateStore {
    type Error;

    fn diff(
        &mut self,
        _: impl Iterator<Item = Migration>,
    ) -> Result<&dyn Iterator<Item = Migration>, Self::Error>;
}

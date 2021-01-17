pub trait MigrationStateStore {
    type Error;
    type Migration;
    type MigrationState;

    fn diff(
        &mut self,
        _: Vec<Self::Migration>,
    ) -> Result<Vec<Self::MigrationState>, Self::Error>;
}
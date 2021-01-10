pub trait MigrationStateStore {
    type Error;
    type Migration;
    type MigrationState;

    fn filter(
        &mut self,
        migrations: Vec<Self::Migration>,
    ) -> Result<Vec<Self::MigrationState>, Self::Error>;
}

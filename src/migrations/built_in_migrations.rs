use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "src/migrations/"]
#[prefix = "src/migrations/"]
pub struct BuiltInMigrations;

use crate::filename;
use crate::migrations;
use crate::migrations::{Direction, Migration};
use rust_embed::RustEmbed;
use std::path::Path;

#[derive(RustEmbed)]
#[folder = "src/migrations/"]
struct BuiltInMigrations;

pub fn built_in_migrations() -> Vec<Migration> {
    let mut m = Vec::new();
    for file in BuiltInMigrations::iter() {
        let f = file.into_owned();
        match filename::parse(Path::new(&f)) {
            Ok(file_name) => m.push(Migration {
                parsed: file_name,
                direction: migrations::Direction::Up,
            }),
            Err(e) => {
                trace!("{:?}", e);
                continue;
            }
        };
    }
    return m;
}

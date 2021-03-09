#[macro_use]
extern crate log;

pub mod config;
pub mod exit_code;
pub mod migrations;
pub mod reserved;
pub mod runner;
pub mod state_store;
pub mod ui;

#[cfg(test)]
#[ctor::ctor]
fn init() {
    env_logger::Builder::new()
        .filter(None, log::LevelFilter::Info)
        .parse_env("MITRE_TEST_LOG")
        .init();
}

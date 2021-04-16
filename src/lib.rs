#[macro_use]
extern crate log; // TODO: replace this with a use() statement?

pub mod config;
pub mod exit_code;
pub mod ffi;
pub mod migrations;
pub mod reserved;
pub mod runner;
pub mod state_store;
pub mod ui;

#[cfg(test)]
#[ctor::ctor]
fn init() {
    env_logger::Builder::new()
        .filter(None, log::LevelFilter::Off)
        .parse_env("MITRE_TEST_LOG")
        .init();
}

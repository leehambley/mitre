extern crate env_logger;
#[macro_use]
extern crate log;

use clap::{App, Arg};
#[macro_use]
extern crate prettytable;
use prettytable::{Cell, Row, Table};
use std::path::Path;

mod built_in_migrations;
mod config;
mod filename;
mod migration_state_store;
mod migrations;
mod reserved;
mod runner;

//
// TODO: move nearly all of this to a "mitre.rs" because
// it should be part of a public crate interface, too
//

use migration_state_store::MigrationStateStore;
use runner::mariadb::MariaDB;
use runner::Runner;
use std::process::exit;

pub const DEFAULT_CONFIG_FILE: &'static str = "mitre.yml";
pub const DEFAULT_MIGRATIONS_DIR: &'static str = ".";

// Constants here maybe should exposed over FFI too
// so that binding libraries can reproduce them
// without hard-coded magic numbers.
//
// std::process::exit expects i32, but I prefer u8
// as I know that's what unix wants, I don't know
// about other platforms.
pub const CONFIG_PROBLEM_EXIT_CODE: u8 = 125;
pub const MIGRATION_DIR_PROBLEM_CODE: u8 = 126;
pub const NO_MITRE_CONFIG_SPECIFIED_EXIT_CODE: u8 = 127;
pub const MITRE_CONFIG_PROBLEM_EXIT_CODE: u8 = 128;
pub const MITRE_STATE_STORE_PROBLEM_EXIT_CODE: u8 = 129;

fn main() {
    env_logger::init();

    let m = App::new("mitre")
        .version("0.1")
        .author("Lee Hambley <lee.hambley@gmail.com>")
        .about("CLI runner for migrations")
        .arg(
            Arg::with_name("config_file")
                .long("config")
                .short('c')
                .takes_value(true)
                .value_name("CONFIG FILE")
                .about("The configuration file to use"),
        )
        .arg(
            Arg::with_name("directory")
                .long("directory")
                .short('d')
                .takes_value(true)
                .value_name("MIGRATION DIR")
                .about("The directory to use"),
        )
        .subcommand(
            App::new("reserved-words")
                .about("utilties for reserved words")
                .subcommand(App::new("ls").about("list reserved words")),
        )
        .subcommand(App::new("ls").about("list all migrations and their status"))
        .subcommand(App::new("show-config").about("for showing config file"))
        .subcommand(App::new("show-migrations").about("for migrations"))
        .get_matches();

    match m.subcommand_name() {
        Some("reserved-words") => {
            let mut table = Table::new();
            table.add_row(row!["Word", "Kind", "Reason"]);
            reserved::words().iter().for_each(|word| {
                table.add_row(Row::new(vec![
                    Cell::new(word.word).style_spec("bFy"),
                    Cell::new(&word.kind.to_string()).style_spec("Fb"),
                    Cell::new(word.reason),
                ]));
            });
            table.printstd();
        }

        // Some("show-config") => {
        //     if let Some(ref config_file) = m.value_of("config_file") {
        //         let path = Path::new(config_file);
        //         match config::from_file(path) {
        //             Ok(c) => {
        //                 let mitre_config = c.get("mitre").expect("must provide mitre config");
        //                 let mdb = MariaDB::new(mitre_config);
        //                 match mdb {
        //                     Ok(mut mmmdb) => {
        //                         println!("bootstrap {:?}", mmmdb.bootstrap());
        //                     }
        //                     Err(e) => {
        //                         println!("error connecting/reading config for mariadb {:?}", e);
        //                         std::process::exit(123);
        //                     }
        //                 };
        //             }
        //             Err(e) => println!("error loading config: {:?}", e),
        //         };
        //         println!("using {:?}", path);
        //     }
        // }
        Some("ls") => {
            let _migrations = match migrations::migrations(Path::new(
                m.value_of("directory").unwrap_or(DEFAULT_MIGRATIONS_DIR),
            )) {
                Ok(m) => built_in_migrations::built_in_migrations().extend(m.iter().cloned()),
                Err(_) => {
                    println!("there was a problem enumerating migrations in that dir");
                    exit(MIGRATION_DIR_PROBLEM_CODE as i32);
                }
            };

            let config = match config::from_file(Path::new(
                m.value_of("config_file").unwrap_or(DEFAULT_CONFIG_FILE),
            )) {
                Ok(c) => c,
                Err(e) => {
                    println!("error loading config: {:?}", e);
                    exit(CONFIG_PROBLEM_EXIT_CODE as i32);
                }
            };

            let mitre_config = match config.get("mitre") {
                Some(mc) => mc,
                None => {
                    println!("no config found for mitre");
                    exit(NO_MITRE_CONFIG_SPECIFIED_EXIT_CODE as i32);
                }
            };

            match mitre_config.validate() {
                Err(problems) => {
                    for problem in problems.iter() {
                        println!("Config Problem: {:?}", problem);
                        exit(MITRE_CONFIG_PROBLEM_EXIT_CODE as i32);
                    }
                }
                _ => {}
            }

            let mdb = match MariaDB::new(mitre_config) {
                Ok(mut mdb) => {
                    println!("bootstrap {:?}", mdb.bootstrap());
                    mdb
                }
                Err(e) => {
                    println!("error connecting/reading config for mariadb {:?}", e);
                    std::process::exit(MITRE_STATE_STORE_PROBLEM_EXIT_CODE as i32);
                }
            };
        }
        // let mitre_config = c.get("mitre").expect("must provide mitre config");
        // let mdb = MariaDB::new(mitre_config);
        //           // let runner: &dyn runner::Runner<Error = mariadb::Error> = mdb.clone();
        //           // let store: &dyn migration_state_store::MigrationStateStore = mdb;
        //           match mdb {
        //               Ok(mut mdb) => {
        //                   println!("bootstrap {:?}", mdb.bootstrap());

        //                   // get list of migrations
        //                   // let migrations = match migrations::migrations(path) {
        //                   //   Ok(m) => m,
        //                   //   Err(_) => panic!("something happen"),
        //                   // };

        //                   let migrations: Vec<migrations::Migration> = Vec::new();

        //                   // let mss: &dyn migration_state_store::MigrationStateStore = mdb;
        //                   match mdb.diff(migrations) {
        //                     Ok(_) => println!("migrations diff'ed ok"),
        //                     Err(e) => println!("migrations not diffed ok: {:?}", e)
        //                   }

        //               }
        //               Err(e) => {
        //                   println!("error connecting/reading config for mariadb {:?}", e);
        //                   std::process::exit(123);
        //               }
        //           }
        //       }
        Some("show-migrations") => {
            info!("showing migrations");
            if let Some(ref matches) = m.subcommand_matches("show-migrations") {
                assert!(matches.is_present("directory"));
                let path = Path::new(matches.value_of("directory").unwrap());
                let migrations = match migrations::migrations(path) {
                    Ok(m) => m,
                    Err(_) => panic!("something happen"),
                };

                let mut table = Table::new();
                table.add_row(row!["Filename", "Date/Time", "Flags"]);
                migrations.iter().for_each(|migration| {
                    eprintln!("{:?}", migration);
                    table.add_row(Row::new(vec![
                        Cell::new(migration.parsed.path.to_str().unwrap()).style_spec("bFy"),
                        Cell::new(&format!("{}", migration.parsed.date_time.timestamp())[..])
                            .style_spec("Fb"),
                        Cell::new(&format!("{:?}", migration.parsed.flags)[..]),
                    ]));
                });
                table.printstd();
            }
        }
        Some("up") => {}   // up was used
        Some("down") => {} // down was used
        Some("redo") => {} // redo was used
        _ => {}            // Either no subcommand or one not tested for...
    }
}

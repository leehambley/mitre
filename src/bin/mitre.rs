use clap::{App, Arg};
use log::{info, trace};
use prettytable::{row, *};
use prettytable::{Cell, Row, Table};
use std::path::Path;

use mitre::config;
use mitre::migrations;
use mitre::reserved;
use mitre::runner::mariadb::MariaDb;
use mitre::runner::Runner;

fn main() {
    env_logger::init();

    trace!("starting");

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
        .subcommand(App::new("up").about("run migrations up to the current timestamp"))
        .subcommand(App::new("show-config").about("for showing config file"))
        .subcommand(App::new("show-migrations").about("for migrations"))
        .get_matches();

    let migrations_dir = Path::new(
        m.value_of("directory")
            .unwrap_or(mitre::DEFAULT_MIGRATIONS_DIR),
    );

    let config_file = Path::new(
        m.value_of("config_file")
            .unwrap_or(mitre::DEFAULT_CONFIG_FILE),
    );

    let config = config::from_file(config_file).expect("cannot read config");

    // Validate config contains a mitre runner

    match m.subcommand_name() {
        Some("reserved-words") => {
            let mut table = Table::new();
            table.add_row(row!["Word", "Kind", "Reason", "(extensions)"]);
            reserved::words().iter().for_each(|word| {
                match word {
                    reserved::ReservedWord::Runner(r) => table.add_row(Row::new(vec![
                        Cell::new(r.name).style_spec("bFy"),
                        Cell::new("runner").style_spec("Fb"),
                        Cell::new(r.desc),
                        Cell::new(&r.exts.join(", ")),
                    ])),
                    reserved::ReservedWord::Flag(f) => table.add_row(Row::new(vec![
                        Cell::new(f.name).style_spec("bFy"),
                        Cell::new("flag").style_spec("Fb"),
                        Cell::new(f.meaning),
                        Cell::new("-"),
                    ])),
                };
            });
            table.printstd();
        }

        Some("show-config") => {
            let _mdb = MariaDb::new(config).expect("must be able to instance mariadb runner");
        }

        Some("ls") => {
            let mut table = Table::new();
            table.add_row(row![
                "Status",
                "Built-In",
                "Timestamp",
                "Path",
                "Runner",
                "Directions"
            ]);

            let mut mdb = MariaDb::new(config).expect("must be able to instance mariadb runner");

            // TODO: return something from error_code module in this crate
            // TODO: sort the migrations, list somehow
            info!("cool dude, no more warnings");
            match migrations::migrations(migrations_dir) {
                Err(e) => panic!("Error: {:?}", e),
                Ok(migrations) => {
                    for (migration_state, m) in mdb.diff(migrations).expect("boom") {
                        m.clone().steps.into_iter().for_each(|(direction, s)| {
                            table.add_row(Row::new(vec![
                                Cell::new(format!("{:?}", migration_state).as_str())
                                    .style_spec("bFy"),
                                Cell::new(format!("{:?}", m.built_in).as_str()).style_spec("bFy"),
                                Cell::new(format!("{:?}", m.date_time).as_str()).style_spec("bFy"),
                                Cell::new(format!("{:?}", s.path).as_str()).style_spec("fB"),
                                Cell::new(s.runner.name).style_spec("fB"),
                                Cell::new(format!("{:?}", direction).as_str()).style_spec("fB"),
                            ]));
                        });
                    }
                }
            };
            table.printstd();
        }

        Some("up") => {
            match migrations::migrations(Path::new(migrations_dir)) {
                Err(e) => panic!("Error: {:?}", e),
                Ok(migrations) => {
                    let mut mdb =
                        MariaDb::new(config).expect("must be able to instance mariadb runner");
                    match mdb.up(migrations) {
                        Ok(_r) => println!("Ran up() successfully"),
                        Err(e) => println!("up() had an error {:?}", e),
                    }
                }
            };
        }

        // Some("ls") => {
        //   let migrations = match migrations::migrations(Path::new(
        //     m.value_of("directory").unwrap_or(DEFAULT_MIGRATIONS_DIR),
        //   )) {
        //         Ok(m) => m
        //         Err(_) => {
        //             println!("there was a problem enumerating migrations in that dir");
        //             exit(MIGRATION_DIR_PROBLEM_CODE as i32);
        //         }
        //     };

        //     let config = match config::from_file(Path::new(
        //         m.value_of("config_file").unwrap_or(DEFAULT_CONFIG_FILE),
        //     )) {
        //         Ok(c) => c,
        //         Err(e) => {
        //             println!("error loading config: {:?}", e);
        //             exit(CONFIG_PROBLEM_EXIT_CODE as i32);
        //         }
        //     };

        //     let mitre_config = match config.get("mitre") {
        //         Some(mc) => mc,
        //         None => {
        //             println!("no config found for mitre");
        //             exit(NO_MITRE_CONFIG_SPECIFIED_EXIT_CODE as i32);
        //         }
        //     };

        //     match mitre_config.validate() {
        //         Err(problems) => {
        //             for problem in problems.iter() {
        //                 println!("Config Problem: {:?}", problem);
        //                 exit(MITRE_CONFIG_PROBLEM_EXIT_CODE as i32);
        //             }
        //         }
        //         _ => {}
        //     }

        //     let mdb = match MariaDb::new(mitre_config) {
        //         Ok(mut mdb) => {
        //             println!("bootstrap {:?}", mdb.bootstrap());
        //             mdb
        //         }
        //         Err(e) => {
        //             println!("error connecting/reading config for mariadb {:?}", e);
        //             std::process::exit(MITRE_STATE_STORE_PROBLEM_EXIT_CODE as i32);
        //         }
        //     };
        // }
        // let mitre_config = c.get("mitre").expect("must provide mitre config");
        // let mdb = MariaDb::new(mitre_config);
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
            // info!("showing migrations");
            // if let Some(ref matches) = m.subcommand_matches("show-migrations") {
            //     assert!(matches.is_present("directory"));
            //     let path = Path::new(matches.value_of("directory").unwrap());
            //     let migrations = match migrations::migrations(path) {
            //         Ok(m) => m,
            //         Err(_) => panic!("something happen"),
            //     };

            //     let mut table = Table::new();
            //     table.add_row(row!["Filename", "Date/Time", "Flags"]);
            //     migrations.iter().for_each(|migration| {
            //         eprintln!("{:?}", migration);
            //         table.add_row(Row::new(vec![
            //             Cell::new(migration.parsed.path.to_str().unwrap()).style_spec("bFy"),
            //             Cell::new(&format!("{}", migration.parsed.date_time.timestamp())[..])
            //                 .style_spec("Fb"),
            //             Cell::new(&format!("{:?}", migration.parsed.flags)[..]),
            //         ]));
            //     });
            //     table.printstd();
            // }
        }
        Some("down") => {} // down was used
        Some("redo") => {} // redo was used
        _ => {}            // Either no subcommand or one not tested for...
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn anything() -> Result<(), &'static str> {
        Ok(())
    }
}

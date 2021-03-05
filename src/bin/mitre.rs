use clap::{App, Arg};
use log::{debug, error, trace, warn};
use prettytable::{row, *};
use prettytable::{Cell, Row, Table};
use std::path::Path;

use mitre::config;
use mitre::migrations;
use mitre::reserved;
use mitre::runner::mariadb::MariaDb;
use mitre::runner::Runner;
use mitre::ui::start_web_ui;

fn main() {
    env_logger::Builder::new()
        .filter(None, log::LevelFilter::Info)
        .parse_env("MITRE_LOG")
        .init();

    trace!("starting");

    let m = App::new("mitre")
        .version("0.1")
        .author("Lee Hambley <lee.hambley@gmail.com>")
        .about("CLI runner for migrations")
        .arg(
            Arg::new("config_file")
                .long("config")
                .short('c')
                .takes_value(true)
                .value_name("CONFIG FILE")
                .about("The configuration file to use"),
        )
        .subcommand(App::new("init").about("creates configuration and migrations directory")).arg(
          Arg::new("directory")
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
        .subcommand(App::new("ui").about("starts the web-based UI"))
        .subcommand(App::new("ls").about("list all migrations and their status"))
        .subcommand(App::new("up").about("run migrations up to the current timestamp"))
        .subcommand(App::new("show-config").about("for showing config file"))
        .subcommand(App::new("show-migrations").about("for migrations"))
        .get_matches();

    let config_file = Path::new(
        m.value_of("config_file")
            .unwrap_or(mitre::config::DEFAULT_CONFIG_FILE),
    );

    let config = match config::from_file(config_file) {
        Ok(c) => c,
        Err(e) => {
            error!(
                "Problem reading configuration file {}: {}",
                config_file.display(),
                e
            );
            std::process::exit(1);
        }
    };

    debug!("Config is {:#?}", config);

    // Validate config contains a mitre runner

    match m.subcommand_name() {
        Some("init") => {
            let config_path = m
                .value_of("config_file")
                .unwrap_or(mitre::config::DEFAULT_CONFIG_FILE);
            let migrations_dir = m.value_of("directory").unwrap_or("./migrations");

            if !Path::new(config_path).is_file() {
                match config::default_config_to_file(Path::new(config_path)) {
                    Ok(_) => {
                        println!("Created Mitre config at {}", config_path);
                    }
                    Err(e) => {
                        error!("Could not create Mitre config at {}: {}", config_path, e);
                        std::process::exit(1);
                    }
                }
            } else {
                println!("The config file already exists.")
            }

            if !Path::new(migrations_dir).is_dir() {
                match std::fs::create_dir_all(Path::new(migrations_dir)) {
                    Ok(_) => {
                        println!("Created Mitre migrations directory at {}", migrations_dir);
                    }
                    Err(e) => {
                        error!(
                            "Could not create Mitre migrations directory at {}: {}",
                            migrations_dir, e
                        );
                        std::process::exit(1);
                    }
                }

                let migrations_readme = format!(
                    "# Mitre Migrations
This directory contains migrations to be used with [Mitre](https://github.com/leehambley/mitre).
## Getting Started
To run the migrations in this folder run
```sh
# Getting the current state
mitre -c {} -d {} ls
# See all commands
mitre --help
```
",
                    config_path, migrations_dir
                );

                let readme_path = Path::new(migrations_dir).join(Path::new("README.md"));

                match std::fs::write(readme_path, migrations_readme) {
                    Ok(_) => {
                        println!(
                            "For next steps see our getting started section in {}/README.md",
                            migrations_dir
                        );
                    }
                    Err(e) => {
                        error!("Could not create README in migrations directory at {}/README.md: {}", migrations_dir, e);
                        std::process::exit(1);
                    }
                }
            } else {
                println!("Migrations directory already exists")
            }
        }

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
            let _mdb = MariaDb::new_state_store(&config)
                .expect("must be able to instance mariadb state store");
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

            let mut mdb = match MariaDb::new_state_store(&config) {
                Ok(mdb) => Ok(mdb),
                Err(reason) => {
                    warn!("Error instantiating mdb {:?}", reason);
                    Err(reason)
                }
            }
            .expect("must be able to instance mariadb state store");

            // TODO: return something from error_code module in this crate
            // TODO: sort the migrations, list somehow
            match migrations::migrations(&config) {
                Err(e) => error!("Error: {:?}", e),
                Ok(migrations) => {
                    for (migration_state, m) in mdb.diff(migrations).expect("boom") {
                        m.clone().steps.into_iter().for_each(|(direction, s)| {
                            table.add_row(Row::new(vec![
                                Cell::new(format!("{:?}", migration_state).as_str())
                                    .style_spec("bFy"),
                                Cell::new(format!("{:?}", m.built_in).as_str()).style_spec("bFy"),
                                Cell::new(format!("{:?}", m.date_time).as_str()).style_spec("bFy"),
                                Cell::new(format!("{:?}", s.path).as_str()).style_spec("fB"),
                                Cell::new(m.runner_and_config.0.name).style_spec("fB"),
                                Cell::new(format!("{:?}", direction).as_str()).style_spec("fB"),
                            ]));
                        });
                    }
                }
            };
            table.printstd();
        }

        Some("up") => {
            match migrations::migrations(&config) {
                Err(e) => panic!("Error: {:?}", e),
                Ok(migrations) => {
                    let mut mdb = MariaDb::new_state_store(&config)
                        .expect("must be able to instance mariadb state store");
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

        //     let mdb = match MariaDb::new_state_store(mitre_config) {
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
        // let mdb = MariaDb::new_state_store(mitre_config);
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
        Some("ui") => {
            info!("Starting webserver");
            match migrations::migrations(Path::new(migrations_dir)) {
                Ok(migrations) => {
                    info!("Opening webserver");
                    // TODO: Add a flag to enable / disable open
                    match start_web_ui(
                        Path::new(
                            m.value_of("config_file")
                                .unwrap_or(mitre::DEFAULT_CONFIG_FILE),
                        ),
                        migrations,
                        true,
                    ) {
                        Ok(_) => {
                            info!("Closing webserver")
                        }
                        Err(err) => {
                            info!("Error starting webserver {}", err)
                        }
                    }
                }
                Err(_) => {
                    info!("Error finding migrations")
                }
            }
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

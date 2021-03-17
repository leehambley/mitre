use chrono::Local;
use clap::{crate_authors, App, Arg};
use log::{error, info, trace, warn};
use std::path::Path;
use tabular::{Row, Table};

use mitre::config;
use mitre::migrations;
use mitre::reserved;
use mitre::runner::from_config as runner_from_config;
use mitre::runner::mariadb::MariaDb;
use mitre::state_store::StateStore;
use mitre::ui::start_web_ui;

fn main() {
    env_logger::Builder::new()
        .filter(None, log::LevelFilter::Info)
        .parse_env("MITRE_LOG")
        .init();

    trace!("starting");

    let m = App::new("mitre")
        .version("0.1")
        .author(crate_authors!("\n"))
        .about("CLI runner for migrations")
        .arg(
            Arg::new("config_file")
                .long("config")
                .short('c')
                .takes_value(true)
                .value_name("CONFIG FILE")
                .about("The configuration file to use"),
        )
        .subcommand(App::new("init").about("creates configuration and migrations directory"))
        .arg(
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
        .subcommand(App::new("up").about("deprecated, use migrate"))
        .subcommand(App::new("migrate").about("run all outstanding migrations"))
        .subcommand(App::new("show-config").about("for showing config file"))
        .subcommand(App::new("show-migrations").about("for migrations"))
        .subcommand(
            App::new("generate-migration")
                .about("generates a boilerplate migration for you")
                .arg(
                    Arg::new("name")
                        .long("name")
                        .takes_value(true)
                        .value_name("MIGRATION NAME")
                        .required(true)
                        .about("Name of the migration"),
                )
                .arg(
                    Arg::new("config")
                        .long("config-name")
                        .takes_value(true)
                        .value_name("CONFIG NAME")
                        .required(true)
                        .about("The configuration name (key) you want to generate the migration for from the configured runners"),
                ),
        )
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
                        error!(
                            "Could not create README in migrations directory at {}/README.md: {}",
                            migrations_dir, e
                        );
                        std::process::exit(1);
                    }
                }
            } else {
                println!("Migrations directory already exists")
            }
        }

        Some("reserved-words") => {
            let mut table = Table::new("{:<} {:<} {:<} {:<}");
            reserved::words().iter().for_each(|word| {
                match word {
                    reserved::ReservedWord::Runner(r) => table.add_row(
                        Row::new()
                            .with_cell(r.name)
                            .with_cell("runner")
                            .with_cell(r.desc)
                            .with_cell(&r.exts.join(", ")),
                    ),
                    reserved::ReservedWord::Flag(f) => table.add_row(
                        Row::new()
                            .with_cell(f.name)
                            .with_cell("flag")
                            .with_cell(f.meaning)
                            .with_cell("-"),
                    ),
                };
            });
            print!("{}", table);
        }

        Some("show-config") => {
            let _mdb = MariaDb::new_state_store(&config)
                .expect("must be able to instance mariadb state store");
        }

        Some("ls") => {
            let mut table = Table::new("{:<} {:<} {:<} {:<} {:<} {:<} {:<}");

            table.add_row(
                Row::new()
                    .with_cell("Status")
                    .with_cell("Built-in")
                    .with_cell("Timestamp")
                    .with_cell("Filename")
                    .with_cell("Runner")
                    .with_cell("Tags")
                    .with_cell("Direction"),
            );

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
                            table.add_row(
                                Row::new()
                                    .with_cell(format!("{}", migration_state))
                                    .with_cell(format!("{:?}", m.built_in).as_str())
                                    .with_cell(
                                        m.date_time
                                            .format(crate::migrations::FORMAT_STR)
                                            .to_string(),
                                    )
                                    .with_cell(s.path.into_os_string().into_string().unwrap())
                                    .with_cell(m.runner_and_config.0.name)
                                    .with_cell(
                                        m.flags
                                            .iter()
                                            .map(|f| format!("{}", f))
                                            .collect::<Vec<String>>()
                                            .join(", "),
                                    )
                                    .with_cell(format!("{:?}", direction).as_str()),
                            );
                        });
                    }
                }
            };
            print!("{}", table);
        }

        Some("up") => {
            error!("the 'up' command has become the 'migrate' command, please use that now");
            std::process::exit(1);
        }

        Some("migrate") => {
            match migrations::migrations(&config) {
                Err(e) => panic!("Error: {:?}", e),
                Ok(migrations) => {
                    let mut mdb = MariaDb::new_state_store(&config)
                        .expect("must be able to instance mariadb state store");
                    match mdb.up(migrations) {
                        Ok(r) => {
                            let mut table = Table::new("{:>}  {:<}");
                            for (result, migration) in r {
                                table.add_row(
                                    Row::new().with_cell(format!("{:?}", result)).with_cell(
                                        migration
                                            .date_time
                                            .format(crate::migrations::FORMAT_STR)
                                            .to_string(),
                                    ),
                                );
                            }
                            print!("{}", table);
                        }
                        Err(e) => println!("up() had an error {:?}", e),
                    }
                }
            };
        }

        Some("ui") => {
            info!("Starting webserver");
            match migrations::migrations(&config) {
                Ok(migrations) => {
                    info!("Opening webserver");
                    // TODO: Add a flag to enable / disable open
                    match start_web_ui(config_file.to_path_buf(), migrations, true) {
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
        Some("generate-migration") => {
            info!("generating migration");
            let sub_m = m
                .subcommand_matches("generate-migration")
                .expect("expected to match subcommand");
            let name = sub_m.value_of("name").expect("expected name argument");
            let key = sub_m.value_of("config").expect("expected config argument");

            let migrations_dir = Path::new(
                m.value_of("directory")
                    .unwrap_or(mitre::config::DEFAULT_MIGRATIONS_DIR),
            );

            let config_file = Path::new(
                m.value_of("config_file")
                    .unwrap_or(mitre::config::DEFAULT_CONFIG_FILE),
            );

            let config = config::from_file(config_file).expect("cannot read config");

            match config.get(key) {
                Some(runner_config) => {
                    let timestamp = Local::now().format(crate::migrations::FORMAT_STR);

                    let runner =
                        runner_from_config(runner_config).expect("could not create runner");
                    let (up_template, down_template, extension) = runner.migration_template();
                    let target_path = migrations_dir.join(
                        format!(
                            "{}_{}.{}",
                            timestamp,
                            inflections::case::to_snake_case(name),
                            key
                        )
                        .as_str(),
                    );
                    let up_target_path = target_path.join(format!("up.{}", extension).as_str());

                    let down_target_path = target_path.join(format!("down.{}", extension).as_str());
                    info!(
                        "Generating migration into {}",
                        target_path
                            .to_str()
                            .expect("could not transform target_path to string")
                    );

                    match std::fs::create_dir(target_path) {
                        Ok(_) => match std::fs::write(up_target_path, up_template) {
                            Ok(_) => match std::fs::write(down_target_path, down_template) {
                                Ok(_) => {
                                    info!("Generation done")
                                }
                                Err(e) => {
                                    panic!("Could not write file: {}", e)
                                }
                            },
                            Err(e) => {
                                panic!("Could not write file: {}", e)
                            }
                        },
                        Err(e) => {
                            panic!("Could create dir: {}", e)
                        }
                    }
                }
                None => {
                    panic!("Could not find key {} in config", key)
                }
            }
        }
        _ => {} // Either no subcommand or one not tested for...
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn anything() -> Result<(), &'static str> {
        Ok(())
    }
}

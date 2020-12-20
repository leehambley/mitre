extern crate env_logger;
#[macro_use]
extern crate log;

use clap::{App, Arg};
#[macro_use]
extern crate prettytable;
use prettytable::{Cell, Row, Table};
use std::path::Path;

mod config;
mod filename;
mod migrations;
mod reserved;
mod runner;

use runner::mariadb::MariaDB;
use runner::Runner;

fn main() {
  env_logger::init();

  let m = App::new("mitre")
    .version("0.1")
    .author("Lee Hambley <lee.hambley@gmail.com>")
    .about("CLI runner for migrations")
    .subcommand(
      App::new("reserved-words")
        .about("utilties for reserved words")
        .subcommand(App::new("ls").about("list reserved words")),
    )
    .subcommand(
      App::new("show-config")
        .about("for showing config file")
        .arg(
          Arg::with_name("config_file")
            .long("config")
            .short('c')
            .takes_value(true)
            .value_name("CONFIG FILE")
            .about("The configuration file to use, no default"),
        ),
    )
    .subcommand(
      App::new("show-migrations")
        .about("for migrations")
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
        ),
    )
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

    Some("show-config") => {
      if let Some(ref matches) = m.subcommand_matches("show-config") {
        assert!(matches.is_present("config_file"));
        let path = Path::new(matches.value_of("config_file").unwrap());
        match config::from_file(path) {
          Ok(c) => {
            println!("loading config succeeded {:?}", c);
            let mitre_config = c.get("es-mariadb").expect("must provide mitre config");
            let mdb = MariaDB::new(mitre_config);
            match mdb {
              Ok(mut mmmdb) => {
                println!("bootstrap {:?}", mmmdb.bootstrap());
              }
              Err(e) => {
                println!("error connecting/reading config for mariadb {:?}", e);
                std::process::exit(123);
              }
            };
          }
          Err(e) => {
            println!("error loading config: {:?}", e)
          }
        };
        println!("using {:?}", path);
      }
      println!("wat, no config");
    }

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
            Cell::new(&format!("{}", migration.parsed.date_time.timestamp())[..]).style_spec("Fb"),
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

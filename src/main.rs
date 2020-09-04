use clap::{App, Arg, ArgMatches};
#[macro_use]
extern crate prettytable;
use prettytable::{Cell, Row, Table};

mod filename;
mod migrations;
mod reserved;

fn main() {
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

    Some("show-migrations") => {
      if let Some(ref matches) = m.subcommand_matches("show-migrations") {
        assert!(matches.is_present("directory"));
      }
    }
    Some("up") => {}   // up was used
    Some("down") => {} // dowm was used
    Some("redo") => {} // dowm was used
    _ => {}            // Either no subcommand or one not tested for...
  }
}

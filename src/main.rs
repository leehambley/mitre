use clap::App;
#[macro_use]
extern crate prettytable;
use prettytable::{Cell, Row, Table};

mod filename;
mod reserved;

fn main() {
    let app = App::new("mitre")
        .version("0.1")
        .author("Lee Hambley <lee.hambley@gmail.com>")
        .about("CLI runner for migrations")
        .subcommand(
            App::new("reserved-words")
                .about("utilties for reserved words")
                .subcommand(App::new("ls").about("list reserved words")),
        )
        .get_matches();

    match app.subcommand_name() {
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

        Some("up") => {}   // up was used
        Some("down") => {} // dowm was used
        Some("redo") => {} // dowm was used
        _ => {}            // Either no subcommand or one not tested for...
    }
}

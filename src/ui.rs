use crate::config;
use crate::config::Configuration;
use crate::migrations::Migration;
use crate::runner::mariadb::MariaDb;
use crate::runner::Runner;

use actix_web::{web, App, HttpResponse, HttpServer, Result};
use askama::Template;
use std::path::Path;
use std::sync::Mutex;

use webbrowser;

struct MigrationTableRow {
    state: String,
    date_time: String,
    built_in: bool,
    path: String,
    runner_name: String,
    direction: String,
}

#[derive(Template)]
#[template(path = "migrations.html")]
struct MigrationsTemplate<'a> {
    migrations: &'a Vec<MigrationTableRow>,
}

struct AppData {
    migrations: Mutex<Vec<Migration>>,
    runner: Mutex<MariaDb>,
}

async fn index(data: web::Data<AppData>) -> Result<HttpResponse> {
    let migrations = data.migrations.lock().unwrap();
    let mut runner = data.runner.lock().unwrap();

    let mut v: Vec<MigrationTableRow> = Vec::new();

    for (migration_state, m) in runner.diff(migrations.to_vec()).expect("boom") {
        m.clone().steps.into_iter().for_each(|(direction, s)| {
            v.push(MigrationTableRow {
                state: format!("{:?}", migration_state),
                built_in: m.built_in,
                date_time: String::from(format!("{:?}", m.date_time)),
                path: format!("{:?}", s.path),
                runner_name: m.runner_and_config.1._runner.clone(),
                direction: format!("{:?}", direction),
            })
        })
    }

    let template = MigrationsTemplate { migrations: &v };
    Ok(HttpResponse::Ok()
        .content_type("text/html")
        .body(template.render().unwrap()))
}

#[actix_web::main]
pub async fn start_web_ui(
    config_file: &'static Path,
    migrations: Vec<Migration>,
    open: bool,
) -> std::io::Result<()> {
    info!("mig {:?}", migrations);
    let url = "127.0.0.1:8000";
    let server = HttpServer::new(move || {
        App::new()
            .data(AppData {
                migrations: Mutex::new(migrations.clone()),
                runner: Mutex::new(
                    MariaDb::new(config::from_file(config_file).expect("cannot read config"))
                        .expect("must be able to instance mariadb runner"),
                ),
            })
            .route("/", web::get().to(index))
    })
    .bind(url)?
    .run();

    if open {
        let url_with_protocol = format!("http://{}", url);
        match webbrowser::open(url_with_protocol.as_str()) {
            Ok(_) => {
                info!("Browser opened")
            }
            Err(err) => {
                info!("Browser could not be opened: {}", err)
            }
        }
    }

    server.await
}

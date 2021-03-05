use crate::migrations::Migration;
use actix_web::{web, App, HttpResponse, HttpServer, Result};
use askama::Template;
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
}

async fn index(data: web::Data<AppData>) -> Result<HttpResponse> {
    let migrations = &mut data.migrations.lock().unwrap();

    let mut v: Vec<MigrationTableRow> = Vec::new();

    for m in migrations.iter() {
        m.clone().steps.into_iter().for_each(|(direction, s)| {
            v.push(MigrationTableRow {
                state: String::from("N/A"),
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
pub async fn start_web_ui(migrations: Vec<Migration>, open: bool) -> std::io::Result<()> {
    info!("mig {:?}", migrations);
    let url = "127.0.0.1:8000";
    let server = HttpServer::new(move || {
        App::new()
            .data(AppData {
                migrations: Mutex::new(migrations.clone()),
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

use crate::config;
use crate::migrations::Migration;
use crate::state_store::StateStore;
use actix_web::{middleware::Logger, web, App, HttpResponse, HttpServer, Result};
use askama::Template;
use std::path::PathBuf;
use std::sync::Mutex;

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
    state_store: Mutex<Box<StateStore>>,
}

async fn index(data: web::Data<AppData>) -> Result<HttpResponse> {
    let migrations = data.migrations.lock().unwrap();
    let mut state_store = data.state_store.lock().unwrap();

    let mut v: Vec<MigrationTableRow> = Vec::new();

    for (migration_state, m) in state_store.diff(migrations.to_vec()).expect("boom") {
        m.clone().steps.into_iter().for_each(|(direction, s)| {
            v.push(MigrationTableRow {
                state: format!("{:?}", migration_state),
                built_in: m.built_in,
                date_time: format!("{:?}", m.date_time),
                path: format!("{:?}", s.path),
                runner_name: m.configuration_name.to_string(),
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
    config_file: PathBuf,
    migrations: Vec<Migration>,
    open: bool,
) -> Result<(), std::io::Error> {
    info!("mig {:?}", migrations);
    let listen = "127.0.0.1:8000";
    let config = config::from_file(&config_file).expect("could not read config");
    let server = HttpServer::new(move || {
        App::new()
            .wrap(Logger::new("%a %{User-Agent}i %r %s %b %Dms %U"))
            .data(AppData {
                migrations: Mutex::new(migrations.clone()),
                state_store: Mutex::new(Box::new(
                    StateStore::from_config(&config).expect("could not make state store"),
                )),
            })
            .route("/", web::get().to(index))
    })
    .bind(listen)?
    .run();

    if open {
        let url_with_protocol = format!("http://{}", listen);
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

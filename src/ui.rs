use crate::migrations::Migration;
use crate::{
    config, migration_list_from_disk, migration_storage_from_config, Engine, MigrationList,
    MigrationStorage, Configuration
};
use actix_web::{middleware::Logger, web, App, HttpResponse, HttpServer, Result};
use askama::Template;
use log::info;
use std::ops::DerefMut;
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
    migration_list: Mutex<Box<dyn MigrationList>>,
    migration_storage: Mutex<Box<dyn MigrationStorage>>,
}

async fn index(data: web::Data<AppData>) -> Result<HttpResponse> {
    let mut migration_list = data.migration_list.lock().unwrap();
    let mut migration_storage = data.migration_storage.lock().unwrap();

    let mut v: Vec<MigrationTableRow> = Vec::new();

    for (migration_state, m) in
        Engine::diff(migration_list.deref_mut(), migration_storage.deref_mut()).expect("boom")
    {
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
    let server = HttpServer::new(move || {
        
        let config = Box::new(config::from_file(&config_file).expect("could not read config"));
        let c: &'static Configuration = Box::leak(config);

        App::new()
            .wrap(Logger::new("%a %{User-Agent}i %r %s %b %Dms %U"))
            .data(AppData {
                migration_list: Mutex::new(Box::new(migration_list_from_disk(c))),
                migration_storage: Mutex::new(Box::new(
                    migration_storage_from_config(c)
                        .expect("could not make migration storage"),
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

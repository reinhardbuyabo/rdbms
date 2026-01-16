use actix_cors::Cors;
use actix_web::{App, HttpServer, middleware, web};
use anyhow::{Context, Result as AnyhowResult};
use db::engine::Engine;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

mod handlers;
mod models;

use handlers::*;

#[derive(Clone)]
pub struct AppState {
    engine: Arc<Mutex<Engine>>,
    transactions: Arc<Mutex<HashMap<String, Arc<Mutex<Engine>>>>>,
}

#[actix_web::main]
async fn main() -> AnyhowResult<()> {
    env_logger::init();

    let db_path: PathBuf = env::var("DB_PATH")
        .unwrap_or_else(|_| "./data.db".to_string())
        .into();

    let port = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .context("Invalid PORT value")?;

    println!("Starting Actix DB Service");
    println!("Database path: {:?}", db_path);
    println!("Port: {}", port);

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).context("create db directory")?;
    }

    let engine = Arc::new(Mutex::new(
        Engine::new(&db_path).context("Failed to initialize database engine")?,
    ));

    let app_state = AppState {
        engine,
        transactions: Arc::new(Mutex::new(HashMap::new())),
    };

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .wrap(cors)
            .wrap(middleware::Logger::default())
            .service(
                web::scope("/api")
                    .route("/health", web::get().to(health))
                    .route("/sql", web::post().to(execute_sql))
                    .route("/tx/begin", web::post().to(begin_transaction))
                    .route("/tx/{tx_id}/commit", web::post().to(commit_transaction))
                    .route("/tx/{tx_id}/abort", web::post().to(abort_transaction)),
            )
    })
    .bind(("0.0.0.0", port))
    .context("Failed to bind server")?
    .run()
    .await
    .context("Failed to run server")?;

    Ok(())
}

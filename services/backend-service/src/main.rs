use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpServer};
use anyhow::{Context, Result as AnyhowResult};
use clap::Parser;
use db::engine::Engine;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use wal::Transaction;

mod auth;
mod handlers;
mod jwt;
mod models;

use auth::*;
use handlers::*;

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<Mutex<Engine>>,
    pub transactions: Arc<Mutex<HashMap<String, Arc<Mutex<Transaction>>>>>,
}

#[derive(Parser, Debug)]
#[command(name = "backend-service")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Database file path
    #[arg(short, long, default_value = "./data.db")]
    db: PathBuf,

    /// Port to listen on
    #[arg(short, long, default_value = "8080")]
    port: u16,

    /// Bind address (for TCP socket binding)
    #[arg(long, default_value = "0.0.0.0")]
    bind: String,
}

#[actix_web::main]
async fn main() -> AnyhowResult<()> {
    env_logger::init();

    let args = Args::parse();

    // Allow environment variables to override CLI args
    let db_path: PathBuf = if let Ok(path) = env::var("DB_PATH") {
        path.into()
    } else {
        args.db
    };

    let port = if let Ok(port_str) = env::var("PORT") {
        port_str.parse().context("Invalid PORT value")?
    } else {
        args.port
    };

    let bind = if let Ok(bind) = env::var("BIND") {
        bind
    } else {
        args.bind
    };

    println!("Starting RDBMS Backend Service");
    println!("Database path: {:?}", db_path);
    println!("Listening on: {}:{}", bind, port);

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

    let bind_addr = format!("{}:{}", bind, port);

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
            .service(
                web::scope("/auth")
                    .route("/google/start", web::get().to(google_auth_start))
                    .route("/google/callback", web::get().to(google_auth_callback)),
            )
            .route("/me", web::get().to(get_me))
    })
    .bind(&bind_addr)
    .context("Failed to bind server")?
    .run()
    .await
    .context("Failed to run server")?;

    Ok(())
}

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

pub mod app_state;
pub mod auth;
pub mod handlers;
pub mod jwt;
pub mod models;

use crate::app_state::AppState;
use crate::auth::{get_me, google_auth_callback, google_auth_start, update_profile, update_role};
use crate::handlers::{
    abort_transaction, begin_transaction, commit_transaction, confirm_order, create_event,
    create_order, create_ticket_type, delete_event, delete_ticket_type, execute_sql, get_event,
    get_order, health, list_events, list_orders, list_ticket_types, list_tickets, publish_event,
    update_event, update_ticket_type, update_user_role,
};

#[derive(Parser, Debug)]
#[command(name = "backend-service")]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "./data.db")]
    db: PathBuf,

    #[arg(short, long, default_value = "8080")]
    port: u16,

    #[arg(long, default_value = "0.0.0.0")]
    bind: String,
}

#[actix_web::main]
async fn main() -> AnyhowResult<()> {
    env_logger::init();

    let args = Args::parse();

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

    // Debug: Print environment variables (mask secrets)
    println!("\n=== Environment Debug ===");
    println!(
        "GOOGLE_CLIENT_ID: {}",
        std::env::var("GOOGLE_CLIENT_ID").unwrap_or_else(|_| "NOT SET".to_string())
    );
    println!(
        "GOOGLE_REDIRECT_URI: {}",
        std::env::var("GOOGLE_REDIRECT_URI")
            .unwrap_or_else(|_| "NOT SET (using default)".to_string())
    );
    println!(
        "JWT_SECRET: {}",
        if std::env::var("JWT_SECRET").is_ok() {
            "SET (hidden)"
        } else {
            "NOT SET"
        }
    );
    println!("=========================\n");

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
            .service(
                web::scope("/v1")
                    .route("/users/me", web::get().to(get_me))
                    .route("/users/me/role", web::post().to(update_role))
                    .route("/users/me", web::patch().to(update_profile))
                    .route(
                        "/admin/users/{user_id}/role",
                        web::post().to(update_user_role),
                    )
                    .route("/events", web::post().to(create_event))
                    .route("/events", web::get().to(list_events))
                    .route("/events/{event_id}", web::get().to(get_event))
                    .route("/events/{event_id}", web::patch().to(update_event))
                    .route("/events/{event_id}", web::delete().to(delete_event))
                    .route("/events/{event_id}/publish", web::post().to(publish_event))
                    .route(
                        "/events/{event_id}/ticket-types",
                        web::post().to(create_ticket_type),
                    )
                    .route(
                        "/events/{event_id}/ticket-types",
                        web::get().to(list_ticket_types),
                    )
                    .route(
                        "/events/{event_id}/ticket-types/{ticket_type_id}",
                        web::patch().to(update_ticket_type),
                    )
                    .route(
                        "/events/{event_id}/ticket-types/{ticket_type_id}",
                        web::delete().to(delete_ticket_type),
                    )
                    .route("/orders", web::post().to(create_order))
                    .route("/orders", web::get().to(list_orders))
                    .route("/orders/{order_id}", web::get().to(get_order))
                    .route("/orders/{order_id}/confirm", web::post().to(confirm_order))
                    .route("/tickets", web::get().to(list_tickets)),
            )
    })
    .bind(&bind_addr)
    .context("Failed to bind server")?
    .run()
    .await
    .context("Failed to run server")?;

    Ok(())
}

use anyhow::{Context, Result};
use clap::Parser;
use db::engine::Engine;
use db::printer::{ReplOutput, SerializableValue};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Parser, Debug)]
#[command(name = "rdbmsd", about = "RDBMS TCP Server")]
struct Args {
    #[arg(long, value_name = "PATH", default_value = "/data")]
    db: PathBuf,

    #[arg(long, value_name = "ADDR", default_value = "0.0.0.0:5432")]
    listen: SocketAddr,

    #[arg(long)]
    workers: Option<usize>,
}

#[derive(Serialize, Deserialize)]
struct Request {
    method: String,
    params: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
struct Response {
    status: String,
    result: Option<serde_json::Value>,
    error: Option<String>,
}

fn output_to_result(output: ReplOutput) -> serde_json::Value {
    match output {
        ReplOutput::Rows { schema, rows } => {
            let columns: Vec<String> = schema.fields.iter().map(|f| f.name.clone()).collect();
            let rows_serialized: Vec<Vec<SerializableValue>> = rows
                .into_iter()
                .map(|row| {
                    row.values()
                        .iter()
                        .map(|v| SerializableValue::from(v.clone()))
                        .collect()
                })
                .collect();
            serde_json::json!({
                "columns": columns,
                "rows": rows_serialized
            })
        }
        ReplOutput::Message(msg) => serde_json::json!({ "message": msg }),
    }
}

async fn handle_client(mut stream: TcpStream, engine: Arc<Mutex<Engine>>) -> Result<()> {
    let mut buffer = [0u8; 4096];
    let addr = stream.peer_addr()?;

    loop {
        let bytes_read = stream.read(&mut buffer).await?;
        if bytes_read == 0 {
            break;
        }

        let request: Request =
            serde_json::from_slice(&buffer[..bytes_read]).context("parse request")?;

        let response = {
            let mut engine_guard = engine.lock().map_err(|e| anyhow::anyhow!("{}", e))?;
            match request.method.as_str() {
                "execute" => {
                    let sql = request
                        .params
                        .clone()
                        .and_then(|p| p.as_str().map(|s| s.to_string()))
                        .unwrap_or_default();
                    match engine_guard.execute_sql(&sql) {
                        Ok(output) => Response {
                            status: "ok".to_string(),
                            result: Some(output_to_result(output)),
                            error: None,
                        },
                        Err(e) => Response {
                            status: "error".to_string(),
                            result: None,
                            error: Some(e.to_string()),
                        },
                    }
                }
                "ping" => Response {
                    status: "ok".to_string(),
                    result: Some(serde_json::json!({"version": env!("CARGO_PKG_VERSION")})),
                    error: None,
                },
                _ => Response {
                    status: "error".to_string(),
                    result: None,
                    error: Some(format!("unknown method: {}", request.method)),
                },
            }
        };

        let response_json = serde_json::to_vec(&response)?;
        stream.write_all(&response_json).await?;
    }

    println!("Client {} disconnected", addr);
    Ok(())
}

async fn accept_loop(listener: TcpListener, engine: Arc<Mutex<Engine>>) -> Result<()> {
    loop {
        let (stream, addr) = listener.accept().await?;
        println!("Client {} connected", addr);

        let engine = Arc::clone(&engine);
        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, engine).await {
                eprintln!("Error handling client {}: {}", addr, e);
            }
        });
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if let Some(parent) = args.db.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).context("create db directory")?;
    }

    println!("RDBMS Server v{}", env!("CARGO_PKG_VERSION"));
    println!("Database: {}", args.db.display());
    println!("Listening on: {}", args.listen);

    let engine = Arc::new(Mutex::new(Engine::new(&args.db)?));

    let listener = TcpListener::bind(args.listen)
        .await
        .context("bind socket")?;

    if let Some(workers) = args.workers {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(workers)
            .build()?
            .block_on(async { accept_loop(listener, engine).await })
    } else {
        accept_loop(listener, engine).await
    }
}

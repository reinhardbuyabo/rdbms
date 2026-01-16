use actix_web::{HttpResponse, Result, web};
use db::printer::{ReplOutput, SerializableValue};

use crate::AppState;
use crate::models::*;

pub async fn health() -> Result<HttpResponse> {
    let response = HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn execute_sql(
    req: web::Json<SqlRequest>,
    data: web::Data<AppState>,
) -> Result<HttpResponse> {
    let SqlRequest { sql, tx_id } = req.into_inner();

    if let Some(tx_id) = tx_id {
        // Execute within existing transaction
        execute_in_transaction(&data, &tx_id, &sql).await
    } else {
        // Execute with autocommit
        execute_autocommit(&data, &sql).await
    }
}

async fn execute_autocommit(data: &AppState, sql: &str) -> Result<HttpResponse> {
    let mut engine = match data.engine.lock() {
        Ok(engine) => engine,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse {
                error_code: "INTERNAL_ERROR".to_string(),
                message: format!("Failed to acquire engine lock: {}", e),
            }));
        }
    };

    match engine.execute_sql(sql) {
        Ok(output) => {
            let response = convert_repl_output_to_sql_response(output);
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            let error_code = categorize_error(&e);
            Ok(HttpResponse::BadRequest().json(ErrorResponse {
                error_code,
                message: e.to_string(),
            }))
        }
    }
}

async fn execute_in_transaction(data: &AppState, tx_id: &str, sql: &str) -> Result<HttpResponse> {
    // Check if transaction exists
    let transactions = match data.transactions.lock() {
        Ok(transactions) => transactions,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse {
                error_code: "INTERNAL_ERROR".to_string(),
                message: format!("Failed to acquire transactions lock: {}", e),
            }));
        }
    };

    if !transactions.contains_key(tx_id) {
        return Ok(HttpResponse::NotFound().json(ErrorResponse {
            error_code: "TX_NOT_FOUND".to_string(),
            message: format!("Transaction {} not found", tx_id),
        }));
    }

    drop(transactions);

    // For now, we'll execute in the main engine but track the transaction
    // In a more sophisticated implementation, we'd have separate transaction contexts
    let mut engine = match data.engine.lock() {
        Ok(engine) => engine,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse {
                error_code: "INTERNAL_ERROR".to_string(),
                message: format!("Failed to acquire engine lock: {}", e),
            }));
        }
    };

    match engine.execute_sql(sql) {
        Ok(output) => {
            let response = convert_repl_output_to_sql_response(output);
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            let error_code = categorize_error(&e);
            Ok(HttpResponse::BadRequest().json(ErrorResponse {
                error_code,
                message: e.to_string(),
            }))
        }
    }
}

pub async fn begin_transaction(data: web::Data<AppState>) -> Result<HttpResponse> {
    let tx_id = uuid::Uuid::new_v4().to_string();

    // For simplicity, we'll just track the transaction ID
    // In a real implementation, we'd create a separate transaction context
    let mut transactions = match data.transactions.lock() {
        Ok(transactions) => transactions,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse {
                error_code: "INTERNAL_ERROR".to_string(),
                message: format!("Failed to acquire transactions lock: {}", e),
            }));
        }
    };

    // Note: This is a simplified implementation
    // We're not actually creating separate transaction contexts yet
    transactions.insert(tx_id.clone(), data.engine.clone());

    Ok(HttpResponse::Ok().json(TransactionResponse { tx_id }))
}

pub async fn commit_transaction(
    path: web::Path<String>,
    data: web::Data<AppState>,
) -> Result<HttpResponse> {
    let tx_id = path.into_inner();

    let mut transactions = match data.transactions.lock() {
        Ok(transactions) => transactions,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse {
                error_code: "INTERNAL_ERROR".to_string(),
                message: format!("Failed to acquire transactions lock: {}", e),
            }));
        }
    };

    if transactions.remove(&tx_id).is_none() {
        return Ok(HttpResponse::NotFound().json(ErrorResponse {
            error_code: "TX_NOT_FOUND".to_string(),
            message: format!("Transaction {} not found", tx_id),
        }));
    }

    Ok(HttpResponse::Ok().json(SuccessResponse {
        message: "Transaction committed".to_string(),
    }))
}

pub async fn abort_transaction(
    path: web::Path<String>,
    data: web::Data<AppState>,
) -> Result<HttpResponse> {
    let tx_id = path.into_inner();

    let mut transactions = match data.transactions.lock() {
        Ok(transactions) => transactions,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse {
                error_code: "INTERNAL_ERROR".to_string(),
                message: format!("Failed to acquire transactions lock: {}", e),
            }));
        }
    };

    if transactions.remove(&tx_id).is_none() {
        return Ok(HttpResponse::NotFound().json(ErrorResponse {
            error_code: "TX_NOT_FOUND".to_string(),
            message: format!("Transaction {} not found", tx_id),
        }));
    }

    Ok(HttpResponse::Ok().json(SuccessResponse {
        message: "Transaction aborted".to_string(),
    }))
}

fn convert_repl_output_to_sql_response(output: ReplOutput) -> SqlResponse {
    match output {
        ReplOutput::Rows { schema, rows } => {
            let columns = Some(schema.fields.iter().map(|f| f.name.clone()).collect());
            let rows_serialized = Some(
                rows.into_iter()
                    .map(|row| {
                        row.values()
                            .iter()
                            .map(|v| SerializableValue::from(v.clone()))
                            .collect()
                    })
                    .collect(),
            );

            SqlResponse {
                columns,
                rows: rows_serialized,
                rows_affected: None,
                message: None,
            }
        }
        ReplOutput::Message(msg) => {
            // Try to parse rows affected from message
            let rows_affected = if msg.starts_with("INSERT ") {
                msg.split_whitespace().nth(1).and_then(|s| s.parse().ok())
            } else if msg.starts_with("DELETE ") {
                msg.split_whitespace().nth(1).and_then(|s| s.parse().ok())
            } else if msg.starts_with("UPDATE ") {
                msg.split_whitespace().nth(1).and_then(|s| s.parse().ok())
            } else {
                None
            };

            SqlResponse {
                columns: None,
                rows: None,
                rows_affected,
                message: Some(msg),
            }
        }
    }
}

fn categorize_error(error: &anyhow::Error) -> String {
    let error_string = error.to_string().to_lowercase();

    if error_string.contains("sql") || error_string.contains("syntax") {
        "SQL_PARSE_ERROR".to_string()
    } else if error_string.contains("table") && error_string.contains("not found") {
        "CATALOG_ERROR".to_string()
    } else if error_string.contains("constraint") || error_string.contains("duplicate") {
        "CONSTRAINT_VIOLATION".to_string()
    } else if error_string.contains("transaction") {
        "TRANSACTION_ERROR".to_string()
    } else {
        "EXECUTION_ERROR".to_string()
    }
}

use actix_web::{web, HttpResponse, Result};
use db::printer::{ReplOutput, SerializableValue};
use std::sync::Arc;

use crate::models::*;
use crate::AppState;

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
        execute_in_transaction(&data, &tx_id, &sql).await
    } else {
        execute_autocommit(&data, &sql).await
    }
}

async fn execute_autocommit(data: &AppState, sql: &str) -> Result<HttpResponse> {
    let mut engine = data.engine.lock();

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
    let transactions = data.transactions.lock();

    let txn = match transactions.get(tx_id) {
        Some(txn) => txn,
        None => {
            return Ok(HttpResponse::NotFound().json(ErrorResponse {
                error_code: "TX_NOT_FOUND".to_string(),
                message: format!("Transaction {} not found", tx_id),
            }));
        }
    };

    let txn_clone = Arc::clone(txn);
    drop(transactions);

    let mut engine = data.engine.lock();

    match engine.execute_sql_in_transaction(sql, &txn_clone) {
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

    let mut engine = data.engine.lock();

    let txn = match engine.begin_transaction() {
        Ok(txn) => txn,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse {
                error_code: "TX_BEGIN_FAILED".to_string(),
                message: format!("Failed to begin transaction: {}", e),
            }));
        }
    };

    let mut transactions = data.transactions.lock();
    transactions.insert(tx_id.clone(), txn);

    Ok(HttpResponse::Ok().json(TransactionResponse { tx_id }))
}

pub async fn commit_transaction(
    path: web::Path<String>,
    data: web::Data<AppState>,
) -> Result<HttpResponse> {
    let tx_id = path.into_inner();

    let mut transactions = data.transactions.lock();

    let txn = match transactions.remove(&tx_id) {
        Some(txn) => txn,
        None => {
            return Ok(HttpResponse::NotFound().json(ErrorResponse {
                error_code: "TX_NOT_FOUND".to_string(),
                message: format!("Transaction {} not found", tx_id),
            }));
        }
    };

    drop(transactions);

    let mut engine = data.engine.lock();

    if let Err(e) = engine.commit_transaction(&txn) {
        return Ok(HttpResponse::InternalServerError().json(ErrorResponse {
            error_code: "TX_COMMIT_FAILED".to_string(),
            message: format!("Failed to commit transaction: {}", e),
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

    let mut transactions = data.transactions.lock();

    let txn = match transactions.remove(&tx_id) {
        Some(txn) => txn,
        None => {
            return Ok(HttpResponse::NotFound().json(ErrorResponse {
                error_code: "TX_NOT_FOUND".to_string(),
                message: format!("Transaction {} not found", tx_id),
            }));
        }
    };

    drop(transactions);

    let mut engine = data.engine.lock();

    if let Err(e) = engine.abort_transaction(&txn) {
        return Ok(HttpResponse::InternalServerError().json(ErrorResponse {
            error_code: "TX_ABORT_FAILED".to_string(),
            message: format!("Failed to abort transaction: {}", e),
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

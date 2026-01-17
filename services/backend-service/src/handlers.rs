use actix_web::{error::InternalError, web, HttpRequest, HttpResponse, Result};
use anyhow::{anyhow, Context};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

use crate::app_state::AppState;
use crate::auth::{create_tables, load_user_by_id};
use crate::jwt::JwtService;
use crate::models::*;
use db::printer::ReplOutput;

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

async fn extract_user_from_request(req: &HttpRequest, data: &AppState) -> Result<(i64, User)> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok());

    let token = match auth_header {
        Some(header) if header.starts_with("Bearer ") => &header[7..],
        _ => {
            return Err(InternalError::new(
                json!({"error": "AUTH_REQUIRED", "message": "Authorization header with Bearer token required"}),
                actix_web::http::StatusCode::UNAUTHORIZED,
            ).into());
        }
    };

    let jwt_secret = match std::env::var("JWT_SECRET") {
        Ok(secret) => secret,
        Err(_) => {
            return Err(InternalError::new(
                json!({"error": "CONFIG_ERROR", "message": "JWT_SECRET not set"}),
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            )
            .into());
        }
    };

    let jwt_service = JwtService::new(&jwt_secret);

    match jwt_service.verify_token(token) {
        Ok(claims) => {
            let user_id: i64 = match claims.sub.parse() {
                Ok(id) => id,
                Err(_) => {
                    return Err(InternalError::new(
                        json!({"error": "INVALID_TOKEN", "message": "Invalid user ID in token"}),
                        actix_web::http::StatusCode::BAD_REQUEST,
                    )
                    .into());
                }
            };

            match load_user_by_id(data, user_id).await {
                Ok(user) => Ok((user_id, user)),
                Err(_) => Err(InternalError::new(
                    json!({"error": "USER_NOT_FOUND", "message": "User not found"}),
                    actix_web::http::StatusCode::UNAUTHORIZED,
                )
                .into()),
            }
        }
        Err(e) => Err(InternalError::new(
            json!({"error": "INVALID_TOKEN", "message": format!("Invalid token: {}", e)}),
            actix_web::http::StatusCode::UNAUTHORIZED,
        )
        .into()),
    }
}

fn check_organizer_role(user: &User) -> Result<()> {
    match user.role {
        UserRole::ORGANIZER => Ok(()),
        _ => Err(InternalError::new(
            json!({"error": "ROLE_FORBIDDEN", "message": "Organizer role required"}),
            actix_web::http::StatusCode::FORBIDDEN,
        )
        .into()),
    }
}

async fn load_event_by_id(data: &AppState, event_id: i64) -> Result<Event, String> {
    let mut engine = data.engine.lock();
    let sql = format!(
        "SELECT id, organizer_user_id, title, description, venue, location, start_time, end_time, status, created_at, updated_at FROM events WHERE id = {}",
        event_id
    );

    match engine.execute_sql(&sql) {
        Ok(ReplOutput::Rows { mut rows, .. }) => {
            if let Some(row) = rows.pop() {
                load_event_by_db_row(&row).map_err(|e: anyhow::Error| e.to_string())
            } else {
                Err("Event not found".to_string())
            }
        }
        Ok(_) => Err("Unexpected response".to_string()),
        Err(e) => Err(e.to_string()),
    }
}

fn load_event_by_db_row(row: &query::Tuple) -> anyhow::Result<Event> {
    let values = row.values();
    let id = Some(values[0].as_i64()?);
    let organizer_user_id = values[1].as_i64()?;
    let title = values[2].as_str()?.to_string();
    let description = values[3].as_str().ok().map(|s| s.to_string());
    let venue = values[4].as_str().ok().map(|s| s.to_string());
    let location = values[5].as_str().ok().map(|s| s.to_string());
    let start_time = values[6].as_str()?.to_string();
    let end_time = values[7].as_str()?.to_string();
    let status_str = values[8].as_str()?.to_string();
    let status = match status_str.as_str() {
        "PUBLISHED" => EventStatus::PUBLISHED,
        "CANCELLED" => EventStatus::CANCELLED,
        _ => EventStatus::DRAFT,
    };
    let created_at_str = values[9]
        .as_str()
        .map_err(|_| anyhow!("Invalid created_at"))?;
    let created_at_naive = NaiveDateTime::parse_from_str(created_at_str, "%Y-%m-%d %H:%M:%S")
        .map_err(|e| anyhow!(e))?;
    let created_at: DateTime<Utc> = DateTime::from_naive_utc_and_offset(created_at_naive, Utc);
    let updated_at_str = values[10]
        .as_str()
        .map_err(|_| anyhow!("Invalid updated_at"))?;
    let updated_at_naive = NaiveDateTime::parse_from_str(updated_at_str, "%Y-%m-%d %H:%M:%S")
        .map_err(|e| anyhow!(e))?;
    let updated_at: DateTime<Utc> = DateTime::from_naive_utc_and_offset(updated_at_naive, Utc);

    Ok(Event {
        id,
        organizer_user_id,
        title,
        description,
        venue,
        location,
        start_time,
        end_time,
        status,
        created_at,
        updated_at,
    })
}

async fn load_ticket_type_by_id(
    data: &AppState,
    ticket_type_id: i64,
) -> Result<TicketType, String> {
    let mut engine = data.engine.lock();
    let sql = format!(
        "SELECT id, event_id, name, price, capacity, sales_start, sales_end, created_at, updated_at FROM ticket_types WHERE id = {}",
        ticket_type_id
    );

    match engine.execute_sql(&sql) {
        Ok(ReplOutput::Rows { mut rows, .. }) => {
            if let Some(row) = rows.pop() {
                load_ticket_type_by_db_row(&row).map_err(|e: anyhow::Error| e.to_string())
            } else {
                Err("Ticket type not found".to_string())
            }
        }
        Ok(_) => Err("Unexpected response".to_string()),
        Err(e) => Err(e.to_string()),
    }
}

fn load_ticket_type_by_db_row(row: &query::Tuple) -> anyhow::Result<TicketType> {
    let values = row.values();
    let id = Some(values[0].as_i64()?);
    let event_id = values[1].as_i64()?;
    let name = values[2].as_str()?.to_string();
    let price = values[3].as_i64()?;
    let capacity = values[4].as_i64()?;
    let sales_start = values[5].as_str().ok().map(|s| s.to_string());
    let sales_end = values[6].as_str().ok().map(|s| s.to_string());
    let created_at_str = values[7]
        .as_str()
        .map_err(|_| anyhow!("Invalid created_at"))?;
    let created_at_naive = NaiveDateTime::parse_from_str(created_at_str, "%Y-%m-%d %H:%M:%S")
        .map_err(|e| anyhow!(e))?;
    let created_at: DateTime<Utc> = DateTime::from_naive_utc_and_offset(created_at_naive, Utc);
    let updated_at_str = values[8]
        .as_str()
        .map_err(|_| anyhow!("Invalid updated_at"))?;
    let updated_at_naive = NaiveDateTime::parse_from_str(updated_at_str, "%Y-%m-%d %H:%M:%S")
        .map_err(|e| anyhow!(e))?;
    let updated_at: DateTime<Utc> = DateTime::from_naive_utc_and_offset(updated_at_naive, Utc);

    Ok(TicketType {
        id,
        event_id,
        name,
        price,
        capacity,
        sales_start,
        sales_end,
        created_at,
        updated_at,
    })
}

async fn load_order_by_id(data: &AppState, order_id: i64) -> Result<Order, String> {
    let mut engine = data.engine.lock();
    let sql = format!(
        "SELECT id, customer_user_id, status, total_amount, created_at, updated_at FROM orders WHERE id = {}",
        order_id
    );

    match engine.execute_sql(&sql) {
        Ok(ReplOutput::Rows { mut rows, .. }) => {
            if let Some(row) = rows.pop() {
                load_order_by_db_row(&row).map_err(|e: anyhow::Error| e.to_string())
            } else {
                Err("Order not found".to_string())
            }
        }
        Ok(_) => Err("Unexpected response".to_string()),
        Err(e) => Err(e.to_string()),
    }
}

fn load_order_by_db_row(row: &query::Tuple) -> anyhow::Result<Order> {
    let values = row.values();
    let id = Some(values[0].as_i64()?);
    let customer_user_id = values[1].as_i64()?;
    let status_str = values[2].as_str()?.to_string();
    let status = match status_str.as_str() {
        "PAID" => OrderStatus::PAID,
        "CANCELLED" => OrderStatus::CANCELLED,
        _ => OrderStatus::PENDING,
    };
    let total_amount = values[3].as_i64()?;
    let created_at_str = values[4]
        .as_str()
        .map_err(|_| anyhow!("Invalid created_at"))?;
    let created_at_naive = NaiveDateTime::parse_from_str(created_at_str, "%Y-%m-%d %H:%M:%S")
        .map_err(|e| anyhow!(e))?;
    let created_at: DateTime<Utc> = DateTime::from_naive_utc_and_offset(created_at_naive, Utc);
    let updated_at_str = values[5]
        .as_str()
        .map_err(|_| anyhow!("Invalid updated_at"))?;
    let updated_at_naive = NaiveDateTime::parse_from_str(updated_at_str, "%Y-%m-%d %H:%M:%S")
        .map_err(|e| anyhow!(e))?;
    let updated_at: DateTime<Utc> = DateTime::from_naive_utc_and_offset(updated_at_naive, Utc);

    Ok(Order {
        id,
        customer_user_id,
        status,
        total_amount,
        created_at,
        updated_at,
    })
}

pub async fn create_event(
    req: web::Json<CreateEventRequest>,
    data: web::Data<AppState>,
    req_http: HttpRequest,
) -> Result<HttpResponse> {
    let (_, user) = match extract_user_from_request(&req_http, &data).await {
        Ok(u) => u,
        Err(e) => return Err(e),
    };

    if let Err(e) = check_organizer_role(&user) {
        return Err(e);
    }

    let create_req = req.into_inner();
    if create_req.title.is_empty() {
        return Ok(HttpResponse::BadRequest()
            .json(json!({"error": "VALIDATION_ERROR", "message": "Title is required"})));
    }
    if create_req.start_time >= create_req.end_time {
        return Ok(HttpResponse::BadRequest().json(
            json!({"error": "VALIDATION_ERROR", "message": "End time must be after start time"}),
        ));
    }

    let mut engine = data.engine.lock();
    if let Err(e) = create_tables(&mut engine) {
        return Ok(HttpResponse::InternalServerError().json(json!({"error": "DATABASE_ERROR", "message": format!("Failed to create tables: {}", e)})));
    }

    let now = Utc::now().format("%Y-%m-%d %H:%M:%S");
    let insert_sql = format!(
        "INSERT INTO events (organizer_user_id, title, description, venue, location, start_time, end_time, status, created_at, updated_at) VALUES ({}, '{}', {}, {}, {}, '{}', '{}', 'DRAFT', '{}', '{}')",
        user.id.unwrap(),
        escape_sql_string(&create_req.title),
        create_req
            .description
            .as_ref()
            .map(|s| format!("'{}'", escape_sql_string(s)))
            .unwrap_or_else(|| "NULL".to_string()),
        create_req
            .venue
            .as_ref()
            .map(|s| format!("'{}'", escape_sql_string(s)))
            .unwrap_or_else(|| "NULL".to_string()),
        create_req
            .location
            .as_ref()
            .map(|s| format!("'{}'", escape_sql_string(s)))
            .unwrap_or_else(|| "NULL".to_string()),
        create_req.start_time,
        create_req.end_time,
        now,
        now
    );

    match engine.execute_sql(&insert_sql) {
        Ok(_) => {
            let select_sql = format!(
                "SELECT id, organizer_user_id, title, description, venue, location, start_time, end_time, status, created_at, updated_at FROM events WHERE organizer_user_id = {} AND title = '{}' ORDER BY id DESC LIMIT 1",
                user.id.unwrap(),
                escape_sql_string(&create_req.title)
            );
            match engine.execute_sql(&select_sql) {
                Ok(ReplOutput::Rows { mut rows, .. }) => {
                    if let Some(row) = rows.pop() {
                        match load_event_by_db_row(&row) {
                            Ok(event) => Ok(HttpResponse::Created().json(json!({"event": event, "message": "Event created successfully"}))),
                            Err(e) => Ok(HttpResponse::InternalServerError().json(json!({"error": "PARSE_ERROR", "message": format!("Failed to parse event: {}", e)}))),
                        }
                    } else {
                        Ok(HttpResponse::InternalServerError().json(json!({"error": "CREATION_ERROR", "message": "Failed to retrieve created event"})))
                    }
                }
                Ok(_) => Ok(HttpResponse::InternalServerError().json(json!({"error": "CREATION_ERROR", "message": "Unexpected response from database"}))),
                Err(e) => Ok(HttpResponse::InternalServerError().json(json!({"error": "CREATION_ERROR", "message": format!("Failed to query created event: {}", e)}))),
            }
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(
            json!({"error": "CREATION_ERROR", "message": format!("Failed to create event: {}", e)}),
        )),
    }
}

pub async fn list_events(
    query: web::Query<HashMap<String, String>>,
    data: web::Data<AppState>,
) -> Result<HttpResponse> {
    let mut engine = data.engine.lock();
    if let Err(e) = create_tables(&mut engine) {
        return Ok(HttpResponse::InternalServerError().json(json!({"error": "DATABASE_ERROR", "message": format!("Failed to create tables: {}", e)})));
    }

    let mut sql = String::from(
        "SELECT id, organizer_user_id, title, description, venue, location, start_time, end_time, status, created_at, updated_at FROM events WHERE 1=1",
    );
    if let Some(status) = query.get("status") {
        sql.push_str(&format!(" AND status = '{}'", status));
    }
    if let Some(from) = query.get("from") {
        sql.push_str(&format!(" AND start_time >= '{}'", from));
    }
    if let Some(to) = query.get("to") {
        sql.push_str(&format!(" AND end_time <= '{}'", to));
    }
    if let Some(q) = query.get("q") {
        sql.push_str(&format!(
            " AND (title LIKE '%{}%' OR description LIKE '%{}%')",
            escape_sql_string(q),
            escape_sql_string(q)
        ));
    }
    sql.push_str(" ORDER BY start_time ASC, id ASC");
    if let Some(limit) = query.get("limit") {
        if let Ok(lim) = limit.parse::<i64>() {
            sql.push_str(&format!(" LIMIT {}", lim));
        }
    }

    match engine.execute_sql(&sql) {
        Ok(ReplOutput::Rows { mut rows, .. }) => {
            let mut events = Vec::new();
            for row in rows.drain(..) {
                if let Ok(event) = load_event_by_db_row(&row) {
                    events.push(event);
                }
            }
            Ok(HttpResponse::Ok().json(json!({"events": events, "count": events.len()})))
        }
        Ok(_) => Ok(HttpResponse::InternalServerError()
            .json(json!({"error": "QUERY_ERROR", "message": "Unexpected response from database"}))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(
            json!({"error": "QUERY_ERROR", "message": format!("Failed to list events: {}", e)}),
        )),
    }
}

pub async fn get_event(path: web::Path<i64>, data: web::Data<AppState>) -> Result<HttpResponse> {
    let event_id = path.into_inner();

    match load_event_by_id(&data, event_id).await {
        Ok(event) => {
            let ticket_types = match list_ticket_types_for_event(&data, event_id).await {
                Ok(types) => types,
                Err(e) => return Ok(HttpResponse::InternalServerError().json(json!({"error": "QUERY_ERROR", "message": format!("Failed to load ticket types: {}", e)}))),
            };

            let mut sold_map = HashMap::new();
            let mut engine = data.engine.lock();
            let count_sql = format!(
                "SELECT ticket_type_id, COUNT(*) as sold FROM tickets WHERE ticket_type_id IN (SELECT id FROM ticket_types WHERE event_id = {}) AND status IN ('HELD', 'ISSUED') GROUP BY ticket_type_id",
                event_id
            );

            match engine.execute_sql(&count_sql) {
                Ok(ReplOutput::Rows { rows, .. }) => {
                    for row in rows {
                        let values = row.values();
                        if let (Ok(tt_id), Ok(sold)) = (values[0].as_i64(), values[1].as_i64()) {
                            sold_map.insert(tt_id, sold);
                        }
                    }
                }
                Ok(_) => {}
                Err(_) => {}
            }

            let ticket_types_with_availability: Vec<TicketTypeWithAvailability> = ticket_types
                .into_iter()
                .map(|tt| {
                    let sold = sold_map.get(&tt.id.unwrap_or(0)).copied().unwrap_or(0);
                    TicketTypeWithAvailability {
                        id: tt.id,
                        event_id: tt.event_id,
                        name: tt.name,
                        price: tt.price,
                        capacity: tt.capacity,
                        sold,
                        remaining: tt.capacity - sold,
                        sales_start: tt.sales_start,
                        sales_end: tt.sales_end,
                        created_at: tt.created_at,
                        updated_at: tt.updated_at,
                    }
                })
                .collect();

            let event_response = EventWithTicketTypes {
                event,
                ticket_types: ticket_types_with_availability,
            };
            Ok(HttpResponse::Ok().json(event_response))
        }
        Err(_) => Ok(HttpResponse::NotFound()
            .json(json!({"error": "NOT_FOUND", "message": "Event not found"}))),
    }
}

async fn list_ticket_types_for_event(
    data: &AppState,
    event_id: i64,
) -> Result<Vec<TicketType>, String> {
    let mut engine = data.engine.lock();
    let sql = format!(
        "SELECT id, event_id, name, price, capacity, sales_start, sales_end, created_at, updated_at FROM ticket_types WHERE event_id = {}",
        event_id
    );

    match engine.execute_sql(&sql) {
        Ok(ReplOutput::Rows { mut rows, .. }) => {
            let mut ticket_types = Vec::new();
            for row in rows.drain(..) {
                if let Ok(tt) = load_ticket_type_by_db_row(&row) {
                    ticket_types.push(tt);
                }
            }
            Ok(ticket_types)
        }
        _ => Ok(Vec::new()),
    }
}

pub async fn update_event(
    path: web::Path<i64>,
    req: web::Json<UpdateEventRequest>,
    data: web::Data<AppState>,
    req_http: HttpRequest,
) -> Result<HttpResponse> {
    let event_id = path.into_inner();
    let (_, user) = match extract_user_from_request(&req_http, &data).await {
        Ok(u) => u,
        Err(e) => return Err(e),
    };

    if let Err(e) = check_organizer_role(&user) {
        return Err(e);
    }

    let event = match load_event_by_id(&data, event_id).await {
        Ok(e) => e,
        Err(_) => {
            return Ok(HttpResponse::NotFound()
                .json(json!({"error": "NOT_FOUND", "message": "Event not found"})));
        }
    };

    if event.organizer_user_id != user.id.unwrap() {
        return Ok(HttpResponse::Forbidden()
            .json(json!({"error": "NOT_OWNER", "message": "You do not own this event"})));
    }

    let update_req = req.into_inner();
    let mut updates = Vec::new();

    if let Some(ref title) = update_req.title {
        if !title.is_empty() {
            updates.push(format!("title = '{}'", escape_sql_string(title)));
        }
    }
    if let Some(ref description) = update_req.description {
        updates.push(format!(
            "description = '{}'",
            escape_sql_string(description)
        ));
    }
    if let Some(ref venue) = update_req.venue {
        updates.push(format!("venue = '{}'", escape_sql_string(venue)));
    }
    if let Some(ref location) = update_req.location {
        updates.push(format!("location = '{}'", escape_sql_string(location)));
    }
    if let Some(ref start_time) = update_req.start_time {
        updates.push(format!("start_time = '{}'", start_time));
    }
    if let Some(ref end_time) = update_req.end_time {
        updates.push(format!("end_time = '{}'", end_time));
    }

    if updates.is_empty() {
        return Ok(HttpResponse::BadRequest()
            .json(json!({"error": "VALIDATION_ERROR", "message": "No fields to update"})));
    }

    updates.push(format!(
        "updated_at = '{}'",
        Utc::now().format("%Y-%m-%d %H:%M:%S")
    ));

    let mut engine = data.engine.lock();
    let sql = format!(
        "UPDATE events SET {} WHERE id = {}",
        updates.join(", "),
        event_id
    );

    match engine.execute_sql(&sql) {
        Ok(_) => {
            match load_event_by_id(&data, event_id).await {
                Ok(updated_event) => Ok(HttpResponse::Ok().json(json!({"event": updated_event, "message": "Event updated successfully"}))),
                Err(e) => Ok(HttpResponse::InternalServerError().json(json!({"error": "UPDATE_ERROR", "message": format!("Failed to reload event: {}", e)}))),
            }
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(json!({"error": "UPDATE_ERROR", "message": format!("Failed to update event: {}", e)}))),
    }
}

pub async fn delete_event(
    path: web::Path<i64>,
    data: web::Data<AppState>,
    req_http: HttpRequest,
) -> Result<HttpResponse> {
    let event_id = path.into_inner();
    let (_, user) = match extract_user_from_request(&req_http, &data).await {
        Ok(u) => u,
        Err(e) => return Err(e),
    };

    if let Err(e) = check_organizer_role(&user) {
        return Err(e);
    }

    let event = match load_event_by_id(&data, event_id).await {
        Ok(e) => e,
        Err(_) => {
            return Ok(HttpResponse::NotFound()
                .json(json!({"error": "NOT_FOUND", "message": "Event not found"})));
        }
    };

    if event.organizer_user_id != user.id.unwrap() {
        return Ok(HttpResponse::Forbidden()
            .json(json!({"error": "NOT_OWNER", "message": "You do not own this event"})));
    }

    let mut engine = data.engine.lock();
    let sql = format!("DELETE FROM events WHERE id = {}", event_id);

    match engine.execute_sql(&sql) {
        Ok(_) => Ok(HttpResponse::NoContent().finish()),
        Err(e) => Ok(HttpResponse::InternalServerError().json(
            json!({"error": "DELETE_ERROR", "message": format!("Failed to delete event: {}", e)}),
        )),
    }
}

pub async fn publish_event(
    path: web::Path<i64>,
    data: web::Data<AppState>,
    req_http: HttpRequest,
) -> Result<HttpResponse> {
    let event_id = path.into_inner();
    let (_, user) = match extract_user_from_request(&req_http, &data).await {
        Ok(u) => u,
        Err(e) => return Err(e),
    };

    if let Err(e) = check_organizer_role(&user) {
        return Err(e);
    }

    let event = match load_event_by_id(&data, event_id).await {
        Ok(e) => e,
        Err(_) => {
            return Ok(HttpResponse::NotFound()
                .json(json!({"error": "NOT_FOUND", "message": "Event not found"})));
        }
    };

    if event.organizer_user_id != user.id.unwrap() {
        return Ok(HttpResponse::Forbidden()
            .json(json!({"error": "NOT_OWNER", "message": "You do not own this event"})));
    }

    let mut engine = data.engine.lock();
    let sql = format!(
        "UPDATE events SET status = 'PUBLISHED', updated_at = '{}' WHERE id = {}",
        Utc::now().format("%Y-%m-%d %H:%M:%S"),
        event_id
    );

    match engine.execute_sql(&sql) {
        Ok(_) => {
            match load_event_by_id(&data, event_id).await {
                Ok(updated_event) => Ok(HttpResponse::Ok().json(json!({"event": updated_event, "message": "Event published successfully"}))),
                Err(e) => Ok(HttpResponse::InternalServerError().json(json!({"error": "PUBLISH_ERROR", "message": format!("Failed to reload event: {}", e)}))),
            }
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(json!({"error": "PUBLISH_ERROR", "message": format!("Failed to publish event: {}", e)}))),
    }
}

pub async fn create_ticket_type(
    path: web::Path<i64>,
    req: web::Json<CreateTicketTypeRequest>,
    data: web::Data<AppState>,
    req_http: HttpRequest,
) -> Result<HttpResponse> {
    let event_id = path.into_inner();
    let (_, user) = match extract_user_from_request(&req_http, &data).await {
        Ok(u) => u,
        Err(e) => return Err(e),
    };

    if let Err(e) = check_organizer_role(&user) {
        return Err(e);
    }

    let event = match load_event_by_id(&data, event_id).await {
        Ok(e) => e,
        Err(_) => {
            return Ok(HttpResponse::NotFound()
                .json(json!({"error": "NOT_FOUND", "message": "Event not found"})));
        }
    };

    if event.organizer_user_id != user.id.unwrap() {
        return Ok(HttpResponse::Forbidden()
            .json(json!({"error": "NOT_OWNER", "message": "You do not own this event"})));
    }

    let create_req = req.into_inner();
    if create_req.name.is_empty() {
        return Ok(HttpResponse::BadRequest()
            .json(json!({"error": "VALIDATION_ERROR", "message": "Name is required"})));
    }
    if create_req.price < 0 {
        return Ok(HttpResponse::BadRequest()
            .json(json!({"error": "VALIDATION_ERROR", "message": "Price cannot be negative"})));
    }
    if create_req.capacity <= 0 {
        return Ok(HttpResponse::BadRequest()
            .json(json!({"error": "VALIDATION_ERROR", "message": "Capacity must be positive"})));
    }

    let mut engine = data.engine.lock();
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S");
    let insert_sql = format!(
        "INSERT INTO ticket_types (event_id, name, price, capacity, sales_start, sales_end, created_at, updated_at) VALUES ({}, '{}', {}, {}, {}, {}, '{}', '{}')",
        event_id,
        escape_sql_string(&create_req.name),
        create_req.price,
        create_req.capacity,
        create_req
            .sales_start
            .as_ref()
            .map(|s| format!("'{}'", s))
            .unwrap_or_else(|| "NULL".to_string()),
        create_req
            .sales_end
            .as_ref()
            .map(|s| format!("'{}'", s))
            .unwrap_or_else(|| "NULL".to_string()),
        now,
        now
    );

    match engine.execute_sql(&insert_sql) {
        Ok(_) => {
            let select_sql = format!("SELECT id, event_id, name, price, capacity, sales_start, sales_end, created_at, updated_at FROM ticket_types WHERE event_id = {} AND name = '{}' ORDER BY id DESC LIMIT 1", event_id, escape_sql_string(&create_req.name));
            match engine.execute_sql(&select_sql) {
                Ok(ReplOutput::Rows { mut rows, .. }) => {
                    if let Some(row) = rows.pop() {
                        match load_ticket_type_by_db_row(&row) {
                            Ok(tt) => Ok(HttpResponse::Created().json(json!({"ticket_type": tt, "message": "Ticket type created successfully"}))),
                            Err(e) => Ok(HttpResponse::InternalServerError().json(json!({"error": "CREATION_ERROR", "message": format!("Failed to parse ticket type: {}", e)}))),
                        }
                    } else {
                        Ok(HttpResponse::InternalServerError().json(json!({"error": "CREATION_ERROR", "message": "Failed to retrieve created ticket type"})))
                    }
                }
                Ok(_) => Ok(HttpResponse::InternalServerError().json(json!({"error": "CREATION_ERROR", "message": "Unexpected response from database"}))),
                Err(e) => Ok(HttpResponse::InternalServerError().json(json!({"error": "CREATION_ERROR", "message": format!("Failed to query created ticket type: {}", e)}))),
            }
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(json!({"error": "CREATION_ERROR", "message": format!("Failed to create ticket type: {}", e)}))),
    }
}

pub async fn list_ticket_types(
    path: web::Path<i64>,
    data: web::Data<AppState>,
) -> Result<HttpResponse> {
    let event_id = path.into_inner();

    match load_event_by_id(&data, event_id).await {
        Ok(_) => {
            match list_ticket_types_for_event(&data, event_id).await {
                Ok(ticket_types) => Ok(HttpResponse::Ok().json(json!({"ticket_types": ticket_types, "count": ticket_types.len()}))),
                Err(e) => Ok(HttpResponse::InternalServerError().json(json!({"error": "QUERY_ERROR", "message": format!("Failed to load ticket types: {}", e)}))),
            }
        }
        Err(_) => Ok(HttpResponse::NotFound().json(json!({"error": "NOT_FOUND", "message": "Event not found"}))),
    }
}

pub async fn update_ticket_type(
    path: web::Path<(i64, i64)>,
    req: web::Json<UpdateTicketTypeRequest>,
    data: web::Data<AppState>,
    req_http: HttpRequest,
) -> Result<HttpResponse> {
    let (event_id, ticket_type_id) = path.into_inner();
    let (_, user) = match extract_user_from_request(&req_http, &data).await {
        Ok(u) => u,
        Err(e) => return Err(e),
    };

    if let Err(e) = check_organizer_role(&user) {
        return Err(e);
    }

    let event = match load_event_by_id(&data, event_id).await {
        Ok(e) => e,
        Err(_) => {
            return Ok(HttpResponse::NotFound()
                .json(json!({"error": "NOT_FOUND", "message": "Event not found"})));
        }
    };

    if event.organizer_user_id != user.id.unwrap() {
        return Ok(HttpResponse::Forbidden()
            .json(json!({"error": "NOT_OWNER", "message": "You do not own this event"})));
    }

    let ticket_type = match load_ticket_type_by_id(&data, ticket_type_id).await {
        Ok(tt) => tt,
        Err(_) => {
            return Ok(HttpResponse::NotFound()
                .json(json!({"error": "NOT_FOUND", "message": "Ticket type not found"})));
        }
    };

    if ticket_type.event_id != event_id {
        return Ok(HttpResponse::BadRequest().json(
            json!({"error": "MISMATCH", "message": "Ticket type does not belong to this event"}),
        ));
    }

    let update_req = req.into_inner();
    let mut updates = Vec::new();

    if let Some(ref name) = update_req.name {
        if !name.is_empty() {
            updates.push(format!("name = '{}'", escape_sql_string(name)));
        }
    }
    if let Some(price) = update_req.price {
        if price < 0 {
            return Ok(HttpResponse::BadRequest().json(
                json!({"error": "VALIDATION_ERROR", "message": "Price cannot be negative"}),
            ));
        }
        updates.push(format!("price = {}", price));
    }
    if let Some(capacity) = update_req.capacity {
        if capacity <= 0 {
            return Ok(HttpResponse::BadRequest().json(
                json!({"error": "VALIDATION_ERROR", "message": "Capacity must be positive"}),
            ));
        }
        let sold = match get_tickets_sold_for_ticket_type(&data, ticket_type_id).await {
            Ok(s) => s,
            Err(e) => return Ok(HttpResponse::InternalServerError().json(json!({"error": "QUERY_ERROR", "message": format!("Failed to check sold count: {}", e)}))),
        };
        if capacity < sold {
            return Ok(HttpResponse::Conflict().json(json!({"error": "CAPACITY_TOO_LOW", "message": format!("Cannot reduce capacity below sold count ({})", sold)})));
        }
        updates.push(format!("capacity = {}", capacity));
    }

    if updates.is_empty() {
        return Ok(HttpResponse::BadRequest()
            .json(json!({"error": "VALIDATION_ERROR", "message": "No fields to update"})));
    }

    updates.push(format!(
        "updated_at = '{}'",
        Utc::now().format("%Y-%m-%d %H:%M:%S")
    ));

    let mut engine = data.engine.lock();
    let sql = format!(
        "UPDATE ticket_types SET {} WHERE id = {}",
        updates.join(", "),
        ticket_type_id
    );

    match engine.execute_sql(&sql) {
        Ok(_) => {
            match load_ticket_type_by_id(&data, ticket_type_id).await {
                Ok(updated_tt) => Ok(HttpResponse::Ok().json(json!({"ticket_type": updated_tt, "message": "Ticket type updated successfully"}))),
                Err(e) => Ok(HttpResponse::InternalServerError().json(json!({"error": "UPDATE_ERROR", "message": format!("Failed to reload ticket type: {}", e)}))),
            }
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(json!({"error": "UPDATE_ERROR", "message": format!("Failed to update ticket type: {}", e)}))),
    }
}

pub async fn delete_ticket_type(
    path: web::Path<(i64, i64)>,
    data: web::Data<AppState>,
    req_http: HttpRequest,
) -> Result<HttpResponse> {
    let (event_id, ticket_type_id) = path.into_inner();
    let (_, user) = match extract_user_from_request(&req_http, &data).await {
        Ok(u) => u,
        Err(e) => return Err(e),
    };

    if let Err(e) = check_organizer_role(&user) {
        return Err(e);
    }

    let event = match load_event_by_id(&data, event_id).await {
        Ok(e) => e,
        Err(_) => {
            return Ok(HttpResponse::NotFound()
                .json(json!({"error": "NOT_FOUND", "message": "Event not found"})));
        }
    };

    if event.organizer_user_id != user.id.unwrap() {
        return Ok(HttpResponse::Forbidden()
            .json(json!({"error": "NOT_OWNER", "message": "You do not own this event"})));
    }

    let ticket_type = match load_ticket_type_by_id(&data, ticket_type_id).await {
        Ok(tt) => tt,
        Err(_) => {
            return Ok(HttpResponse::NotFound()
                .json(json!({"error": "NOT_FOUND", "message": "Ticket type not found"})));
        }
    };

    if ticket_type.event_id != event_id {
        return Ok(HttpResponse::BadRequest().json(
            json!({"error": "MISMATCH", "message": "Ticket type does not belong to this event"}),
        ));
    }

    let sold = match get_tickets_sold_for_ticket_type(&data, ticket_type_id).await {
        Ok(s) => s,
        Err(e) => return Ok(HttpResponse::InternalServerError().json(json!({"error": "QUERY_ERROR", "message": format!("Failed to check sold count: {}", e)}))),
    };
    if sold > 0 {
        return Ok(HttpResponse::Conflict().json(json!({"error": "CANNOT_DELETE_HAS_SALES", "message": format!("Cannot delete ticket type with {} tickets sold", sold)})));
    }

    let mut engine = data.engine.lock();
    let sql = format!("DELETE FROM ticket_types WHERE id = {}", ticket_type_id);

    match engine.execute_sql(&sql) {
        Ok(_) => Ok(HttpResponse::NoContent().finish()),
        Err(e) => Ok(HttpResponse::InternalServerError().json(json!({"error": "DELETE_ERROR", "message": format!("Failed to delete ticket type: {}", e)}))),
    }
}

async fn get_tickets_sold_for_ticket_type(
    data: &AppState,
    ticket_type_id: i64,
) -> Result<i64, String> {
    let mut engine = data.engine.lock();
    let sql = format!(
        "SELECT COUNT(*) FROM tickets WHERE ticket_type_id = {} AND status IN ('HELD', 'ISSUED')",
        ticket_type_id
    );

    match engine.execute_sql(&sql) {
        Ok(ReplOutput::Rows { mut rows, .. }) => {
            if let Some(row) = rows.pop() {
                row.values()[0].as_i64().map_err(|e| e.to_string())
            } else {
                Ok(0)
            }
        }
        _ => Ok(0),
    }
}

pub async fn create_order(
    req: web::Json<CreateOrderRequest>,
    data: web::Data<AppState>,
    req_http: HttpRequest,
) -> Result<HttpResponse> {
    let (user_id, _) = match extract_user_from_request(&req_http, &data).await {
        Ok(u) => u,
        Err(e) => return Err(e),
    };

    let order_req = req.into_inner();
    if order_req.items.is_empty() {
        return Ok(HttpResponse::BadRequest()
            .json(json!({"error": "VALIDATION_ERROR", "message": "Items array cannot be empty"})));
    }

    let mut engine = data.engine.lock();
    let mut total_amount: i64 = 0;
    let mut ticket_type_ids = Vec::new();

    for item in &order_req.items {
        if item.quantity <= 0 {
            return Ok(HttpResponse::BadRequest().json(
                json!({"error": "VALIDATION_ERROR", "message": "Quantity must be positive"}),
            ));
        }

        let ticket_type = match load_ticket_type_by_id(&data, item.ticket_type_id).await {
            Ok(tt) => tt,
            Err(_) => return Ok(HttpResponse::NotFound().json(json!({"error": "TICKET_TYPE_NOT_FOUND", "message": format!("Ticket type {} not found", item.ticket_type_id)}))),
        };

        let event =
            match load_event_by_id(&data, ticket_type.event_id).await {
                Ok(e) => e,
                Err(_) => return Ok(HttpResponse::InternalServerError().json(
                    json!({"error": "EVENT_ERROR", "message": "Event not found for ticket type"}),
                )),
            };

        match event.status {
            EventStatus::CANCELLED => {
                return Ok(HttpResponse::Conflict().json(
                    json!({"error": "EVENT_NOT_FOR_SALE", "message": "Event is cancelled"}),
                ));
            }
            EventStatus::DRAFT => {
                return Ok(HttpResponse::Conflict().json(
                    json!({"error": "EVENT_NOT_FOR_SALE", "message": "Event is not published"}),
                ));
            }
            EventStatus::PUBLISHED => {}
        }

        let sold = match get_tickets_sold_for_ticket_type(&data, item.ticket_type_id).await {
            Ok(s) => s,
            Err(e) => return Ok(HttpResponse::InternalServerError().json(json!({"error": "QUERY_ERROR", "message": format!("Failed to check availability: {}", e)}))),
        };
        if sold + item.quantity > ticket_type.capacity {
            return Ok(HttpResponse::Conflict().json(json!({"error": "SOLD_OUT", "message": format!("Not enough tickets available. Requested: {}, Available: {}", item.quantity, ticket_type.capacity - sold)})));
        }

        total_amount += ticket_type.price * item.quantity;
        ticket_type_ids.push((ticket_type, item.quantity));
    }

    let now = Utc::now().format("%Y-%m-%d %H:%M:%S");
    let insert_order_sql = format!(
        "INSERT INTO orders (customer_user_id, status, total_amount, created_at, updated_at) VALUES ({}, 'PENDING', {}, '{}', '{}')",
        user_id, total_amount, now, now
    );

    let order_id = match engine.execute_sql(&insert_order_sql) {
        Ok(_) => {
            let select_sql = format!(
                "SELECT id FROM orders WHERE customer_user_id = {} ORDER BY id DESC LIMIT 1",
                user_id
            );
            match engine.execute_sql(&select_sql) {
                Ok(ReplOutput::Rows { mut rows, .. }) => {
                    if let Some(row) = rows.pop() {
                        row.values()[0].as_i64().unwrap_or(0)
                    } else {
                        return Ok(HttpResponse::InternalServerError().json(json!({"error": "CREATION_ERROR", "message": "Failed to retrieve created order"})));
                    }
                }
                Ok(_) => return Ok(HttpResponse::InternalServerError().json(json!({"error": "CREATION_ERROR", "message": "Unexpected response from database"}))),
                Err(e) => return Ok(HttpResponse::InternalServerError().json(json!({"error": "CREATION_ERROR", "message": format!("Failed to query created order: {}", e)}))),
            }
        }
        Err(e) => return Ok(HttpResponse::InternalServerError().json(
            json!({"error": "CREATION_ERROR", "message": format!("Failed to create order: {}", e)}),
        )),
    };

    for (ticket_type, quantity) in ticket_type_ids {
        for _ in 0..quantity {
            let insert_ticket_sql = format!(
                "INSERT INTO tickets (order_id, ticket_type_id, unit_price, status, created_at) VALUES ({}, {}, {}, 'HELD', '{}')",
                order_id,
                ticket_type.id.unwrap(),
                ticket_type.price,
                now
            );
            if let Err(e) = engine.execute_sql(&insert_ticket_sql) {
                let rollback_sql = format!("DELETE FROM orders WHERE id = {}", order_id);
                let _ = engine.execute_sql(&rollback_sql);
                let delete_tickets_sql =
                    format!("DELETE FROM tickets WHERE order_id = {}", order_id);
                let _ = engine.execute_sql(&delete_tickets_sql);
                return Ok(HttpResponse::InternalServerError().json(json!({"error": "CREATION_ERROR", "message": format!("Failed to create tickets: {}", e)})));
            }
        }
    }

    match load_order_by_id(&data, order_id).await {
        Ok(order) => Ok(HttpResponse::Created().json(json!({"order": order, "message": "Order created successfully. Please confirm payment."}))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(json!({"error": "CREATION_ERROR", "message": format!("Failed to reload order: {}", e)}))),
    }
}

pub async fn confirm_order(
    path: web::Path<i64>,
    data: web::Data<AppState>,
    req_http: HttpRequest,
) -> Result<HttpResponse> {
    let order_id = path.into_inner();
    let (user_id, _) = match extract_user_from_request(&req_http, &data).await {
        Ok(u) => u,
        Err(e) => return Err(e),
    };

    let order = match load_order_by_id(&data, order_id).await {
        Ok(o) => o,
        Err(_) => {
            return Ok(HttpResponse::NotFound()
                .json(json!({"error": "NOT_FOUND", "message": "Order not found"})));
        }
    };

    if order.customer_user_id != user_id {
        return Ok(HttpResponse::Forbidden()
            .json(json!({"error": "NOT_OWNER", "message": "You do not own this order"})));
    }

    match order.status {
        OrderStatus::PAID => return Ok(HttpResponse::Ok().json(json!({"order": order, "message": "Order is already paid"}))),
        OrderStatus::CANCELLED => return Ok(HttpResponse::Conflict().json(json!({"error": "ORDER_NOT_CONFIRMABLE", "message": "Cannot confirm a cancelled order"}))),
        OrderStatus::PENDING => {}
    }

    let mut engine = data.engine.lock();
    let sql = format!(
        "UPDATE orders SET status = 'PAID', updated_at = '{}' WHERE id = {}",
        Utc::now().format("%Y-%m-%d %H:%M:%S"),
        order_id
    );
    let update_tickets_sql = format!(
        "UPDATE tickets SET status = 'ISSUED' WHERE order_id = {}",
        order_id
    );

    match engine.execute_sql(&sql) {
        Ok(_) => {
            let _ = engine.execute_sql(&update_tickets_sql);
            match load_order_by_id(&data, order_id).await {
                Ok(updated_order) => Ok(HttpResponse::Ok().json(json!({"order": updated_order, "message": "Order confirmed and payment received"}))),
                Err(e) => Ok(HttpResponse::InternalServerError().json(json!({"error": "CONFIRM_ERROR", "message": format!("Failed to reload order: {}", e)}))),
            }
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(
            json!({"error": "CONFIRM_ERROR", "message": format!("Failed to confirm order: {}", e)}),
        )),
    }
}

pub async fn list_orders(data: web::Data<AppState>, req_http: HttpRequest) -> Result<HttpResponse> {
    let (user_id, _) = match extract_user_from_request(&req_http, &data).await {
        Ok(u) => u,
        Err(e) => return Err(e),
    };

    let mut engine = data.engine.lock();
    let sql = format!(
        "SELECT id, customer_user_id, status, total_amount, created_at, updated_at FROM orders WHERE customer_user_id = {} ORDER BY created_at DESC, id DESC",
        user_id
    );

    match engine.execute_sql(&sql) {
        Ok(ReplOutput::Rows { mut rows, .. }) => {
            let mut orders_with_details = Vec::new();
            for row in rows.drain(..) {
                match load_order_by_db_row(&row) {
                    Ok(order) => match load_tickets_for_order(&data, order.id.unwrap()).await {
                        Ok(tickets) => {
                            orders_with_details.push(OrderWithDetails { order, tickets })
                        }
                        Err(_) => orders_with_details.push(OrderWithDetails {
                            order,
                            tickets: Vec::new(),
                        }),
                    },
                    Err(_) => {}
                }
            }
            Ok(HttpResponse::Ok()
                .json(json!({"orders": orders_with_details, "count": orders_with_details.len()})))
        }
        Ok(_) => {
            Ok(HttpResponse::Ok()
                .json(json!({"orders": Vec::<OrderWithDetails>::new(), "count": 0})))
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(
            json!({"error": "QUERY_ERROR", "message": format!("Failed to list orders: {}", e)}),
        )),
    }
}

pub async fn get_order(
    path: web::Path<i64>,
    data: web::Data<AppState>,
    req_http: HttpRequest,
) -> Result<HttpResponse> {
    let order_id = path.into_inner();
    let (user_id, _) = match extract_user_from_request(&req_http, &data).await {
        Ok(u) => u,
        Err(e) => return Err(e),
    };

    let order = match load_order_by_id(&data, order_id).await {
        Ok(o) => o,
        Err(_) => {
            return Ok(HttpResponse::NotFound()
                .json(json!({"error": "NOT_FOUND", "message": "Order not found"})));
        }
    };

    if order.customer_user_id != user_id {
        return Ok(HttpResponse::Forbidden()
            .json(json!({"error": "NOT_OWNER", "message": "You do not own this order"})));
    }

    let tickets = match load_tickets_for_order(&data, order_id).await {
        Ok(t) => t,
        Err(_) => Vec::new(),
    };
    let order_with_details = OrderWithDetails { order, tickets };

    Ok(HttpResponse::Ok().json(order_with_details))
}

async fn load_tickets_for_order(
    data: &AppState,
    order_id: i64,
) -> Result<Vec<TicketWithDetails>, String> {
    let mut engine = data.engine.lock();
    let sql = format!(
        "SELECT t.id, t.order_id, t.ticket_type_id, t.unit_price, t.status, t.created_at, tt.name as tt_name, tt.price as tt_price, e.id as event_id, e.title as event_title, e.start_time as event_start, e.end_time as event_end, e.venue as event_venue, e.location as event_location FROM tickets t JOIN ticket_types tt ON t.ticket_type_id = tt.id JOIN events e ON tt.event_id = e.id WHERE t.order_id = {}",
        order_id
    );

    match engine.execute_sql(&sql) {
        Ok(ReplOutput::Rows { rows, .. }) => {
            let mut tickets = Vec::new();
            for row in rows {
                let values = row.values();
                let id = Some(values[0].as_i64().map_err(|e| e.to_string())?);
                let order_id_inner = values[1].as_i64().map_err(|e| e.to_string())?;
                let ticket_type_id = values[2].as_i64().map_err(|e| e.to_string())?;
                let unit_price = values[3].as_i64().map_err(|e| e.to_string())?;
                let status_str = values[4].as_str().map_err(|e| e.to_string())?.to_string();
                let status = match status_str.as_str() {
                    "ISSUED" => TicketStatus::ISSUED,
                    "VOID" => TicketStatus::VOID,
                    _ => TicketStatus::HELD,
                };
                let created_at_str = values[5].as_str().map_err(|e| e.to_string())?;
                let created_at_naive =
                    NaiveDateTime::parse_from_str(created_at_str, "%Y-%m-%d %H:%M:%S")
                        .map_err(|e| e.to_string())?;
                let created_at: DateTime<Utc> =
                    DateTime::from_naive_utc_and_offset(created_at_naive, Utc);
                let ticket_type_name = values[6].as_str().map_err(|e| e.to_string())?.to_string();
                let ticket_type_price = values[7].as_i64().map_err(|e| e.to_string())?;
                let event_id = values[8].as_i64().map_err(|e| e.to_string())?;
                let event_title = values[9].as_str().map_err(|e| e.to_string())?.to_string();
                let event_start_time = values[10].as_str().map_err(|e| e.to_string())?.to_string();
                let event_end_time = values[11].as_str().map_err(|e| e.to_string())?.to_string();
                let event_venue = values[12].as_str().ok().map(|s| s.to_string());
                let event_location = values[13].as_str().ok().map(|s| s.to_string());

                tickets.push(TicketWithDetails {
                    id,
                    order_id: order_id_inner,
                    ticket_type_id,
                    unit_price,
                    status,
                    ticket_type_name,
                    ticket_type_price,
                    event_id,
                    event_title,
                    event_start_time,
                    event_end_time,
                    event_venue,
                    event_location,
                    created_at,
                });
            }
            Ok(tickets)
        }
        _ => Ok(Vec::new()),
    }
}

pub async fn list_tickets(
    data: web::Data<AppState>,
    req_http: HttpRequest,
) -> Result<HttpResponse> {
    let (user_id, _) = match extract_user_from_request(&req_http, &data).await {
        Ok(u) => u,
        Err(e) => return Err(e),
    };

    let tickets = match load_all_tickets_for_user(&data, user_id).await {
        Ok(t) => t,
        Err(e) => return Ok(HttpResponse::InternalServerError().json(
            json!({"error": "QUERY_ERROR", "message": format!("Failed to load tickets: {}", e)}),
        )),
    };

    Ok(HttpResponse::Ok().json(json!({"tickets": tickets, "count": tickets.len()})))
}

async fn load_all_tickets_for_user(
    data: &AppState,
    user_id: i64,
) -> Result<Vec<TicketWithDetails>, String> {
    let mut engine = data.engine.lock();
    let sql = format!(
        "SELECT t.id, t.order_id, t.ticket_type_id, t.unit_price, t.status, t.created_at, tt.name as tt_name, tt.price as tt_price, e.id as event_id, e.title as event_title, e.start_time as event_start, e.end_time as event_end, e.venue as event_venue, e.location as event_location FROM tickets t JOIN ticket_types tt ON t.ticket_type_id = tt.id JOIN events e ON tt.event_id = e.id JOIN orders o ON t.order_id = o.id WHERE o.customer_user_id = {}",
        user_id
    );

    match engine.execute_sql(&sql) {
        Ok(ReplOutput::Rows { rows, .. }) => {
            let mut tickets = Vec::new();
            for row in rows {
                let values = row.values();
                let id = Some(values[0].as_i64().map_err(|e| e.to_string())?);
                let order_id_inner = values[1].as_i64().map_err(|e| e.to_string())?;
                let ticket_type_id = values[2].as_i64().map_err(|e| e.to_string())?;
                let unit_price = values[3].as_i64().map_err(|e| e.to_string())?;
                let status_str = values[4].as_str().map_err(|e| e.to_string())?.to_string();
                let status = match status_str.as_str() {
                    "ISSUED" => TicketStatus::ISSUED,
                    "VOID" => TicketStatus::VOID,
                    _ => TicketStatus::HELD,
                };
                let created_at_str = values[5].as_str().map_err(|e| e.to_string())?;
                let created_at_naive =
                    NaiveDateTime::parse_from_str(created_at_str, "%Y-%m-%d %H:%M:%S")
                        .map_err(|e| e.to_string())?;
                let created_at: DateTime<Utc> =
                    DateTime::from_naive_utc_and_offset(created_at_naive, Utc);
                let ticket_type_name = values[6].as_str().map_err(|e| e.to_string())?.to_string();
                let ticket_type_price = values[7].as_i64().map_err(|e| e.to_string())?;
                let event_id = values[8].as_i64().map_err(|e| e.to_string())?;
                let event_title = values[9].as_str().map_err(|e| e.to_string())?.to_string();
                let event_start_time = values[10].as_str().map_err(|e| e.to_string())?.to_string();
                let event_end_time = values[11].as_str().map_err(|e| e.to_string())?.to_string();
                let event_venue = values[12].as_str().ok().map(|s| s.to_string());
                let event_location = values[13].as_str().ok().map(|s| s.to_string());

                tickets.push(TicketWithDetails {
                    id,
                    order_id: order_id_inner,
                    ticket_type_id,
                    unit_price,
                    status,
                    ticket_type_name,
                    ticket_type_price,
                    event_id,
                    event_title,
                    event_start_time,
                    event_end_time,
                    event_venue,
                    event_location,
                    created_at,
                });
            }
            Ok(tickets)
        }
        _ => Ok(Vec::new()),
    }
}

fn escape_sql_string(input: &str) -> String {
    input.replace('\'', "''")
}

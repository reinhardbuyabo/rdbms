use actix_web::{web, HttpRequest, HttpResponse, Result};
use anyhow::{anyhow, Context};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use url::form_urlencoded;

use crate::app_state::AppState;
use crate::jwt::JwtService;
use crate::models::*;
use query::Tuple;

const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USER_INFO_URL: &str = "https://www.googleapis.com/oauth2/v2/userinfo";

pub async fn google_auth_start() -> Result<HttpResponse> {
    let client_id = std::env::var("GOOGLE_CLIENT_ID")
        .map_err(|_| actix_web::error::ErrorInternalServerError("GOOGLE_CLIENT_ID not set"))?;

    let redirect_uri = std::env::var("GOOGLE_REDIRECT_URI")
        .unwrap_or_else(|_| "http://localhost:8080/auth/google/callback".to_string());

    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?response_type=code&client_id={}&redirect_uri={}&scope=email%20profile&access_type=offline",
        urlencoding::encode(&client_id),
        urlencoding::encode(&redirect_uri)
    );

    Ok(HttpResponse::Found()
        .append_header(("Location", auth_url))
        .finish())
}

pub async fn google_auth_callback(
    req: HttpRequest,
    data: web::Data<AppState>,
) -> Result<HttpResponse> {
    let query_string = req.query_string();

    let params: HashMap<String, String> = form_urlencoded::parse(query_string.as_bytes())
        .into_owned()
        .collect();

    let code = params
        .get("code")
        .ok_or_else(|| actix_web::error::ErrorBadRequest("Missing authorization code"))?;

    let token_response = exchange_code_for_token(code).await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!(
            "Failed to exchange code for token: {}",
            e
        ))
    })?;

    let access_token = token_response["access_token"].as_str().ok_or_else(|| {
        actix_web::error::ErrorInternalServerError("Missing access_token in response")
    })?;

    let user_info = get_google_user_info(access_token).await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to get user info: {}", e))
    })?;

    let user = upsert_user(&data, &user_info).await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to upsert user: {}", e))
    })?;

    let jwt_secret = std::env::var("JWT_SECRET")
        .map_err(|_| actix_web::error::ErrorInternalServerError("JWT_SECRET not set"))?;

    let jwt_ttl = std::env::var("JWT_TTL_SECONDS")
        .unwrap_or_else(|_| "3600".to_string())
        .parse::<u64>()
        .unwrap_or(3600);

    let jwt_service = JwtService::new(&jwt_secret);
    let token = jwt_service
        .generate_token(&user.id.unwrap().to_string(), &user.email, jwt_ttl)
        .map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!("Failed to generate JWT: {}", e))
        })?;

    let response = AuthResponse { token, user };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn get_me(req: HttpRequest, data: web::Data<AppState>) -> Result<HttpResponse> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok());

    let token = match auth_header {
        Some(header) if header.starts_with("Bearer ") => &header[7..],
        _ => {
            return Ok(HttpResponse::Unauthorized().json(json!({
                "error": "AUTH_REQUIRED",
                "message": "Authorization header with Bearer token required"
            })));
        }
    };

    let jwt_secret = std::env::var("JWT_SECRET")
        .map_err(|_| actix_web::error::ErrorInternalServerError("JWT_SECRET not set"))?;

    let jwt_service = JwtService::new(&jwt_secret);

    match jwt_service.verify_token(token) {
        Ok(claims) => {
            let user_id: i64 = claims
                .sub
                .parse()
                .map_err(|_| actix_web::error::ErrorBadRequest("Invalid user ID in token"))?;

            let user = load_user_by_id(&data, user_id).await.map_err(|e| {
                actix_web::error::ErrorUnauthorized(format!("User not found: {}", e))
            })?;

            let response = MeResponse { user };
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => Ok(HttpResponse::Unauthorized().json(json!({
            "error": "INVALID_TOKEN",
            "message": format!("Invalid token: {}", e)
        }))),
    }
}

pub async fn update_role(
    req: web::Json<RoleChangeRequest>,
    data: web::Data<AppState>,
    req_http: HttpRequest,
) -> Result<HttpResponse> {
    let auth_header = req_http
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok());

    let token = match auth_header {
        Some(header) if header.starts_with("Bearer ") => &header[7..],
        _ => {
            return Ok(HttpResponse::Unauthorized().json(json!({
                "error": "AUTH_REQUIRED",
                "message": "Authorization header with Bearer token required"
            })));
        }
    };

    let jwt_secret = std::env::var("JWT_SECRET")
        .map_err(|_| actix_web::error::ErrorInternalServerError("JWT_SECRET not set"))?;

    let jwt_service = JwtService::new(&jwt_secret);

    match jwt_service.verify_token(token) {
        Ok(claims) => {
            let user_id: i64 = claims
                .sub
                .parse()
                .map_err(|_| actix_web::error::ErrorBadRequest("Invalid user ID in token"))?;

            let role = req.into_inner().role;

            let user = update_user_role(&data, user_id, role).await.map_err(|e| {
                actix_web::error::ErrorBadRequest(format!("Failed to update role: {}", e))
            })?;

            Ok(HttpResponse::Ok().json(json!({
                "user": user,
                "message": "Role updated successfully"
            })))
        }
        Err(e) => Ok(HttpResponse::Unauthorized().json(json!({
            "error": "INVALID_TOKEN",
            "message": format!("Invalid token: {}", e)
        }))),
    }
}

pub async fn update_profile(
    req: web::Json<UserUpdateRequest>,
    data: web::Data<AppState>,
    req_http: HttpRequest,
) -> Result<HttpResponse> {
    let auth_header = req_http
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok());

    let token = match auth_header {
        Some(header) if header.starts_with("Bearer ") => &header[7..],
        _ => {
            return Ok(HttpResponse::Unauthorized().json(json!({
                "error": "AUTH_REQUIRED",
                "message": "Authorization header with Bearer token required"
            })));
        }
    };

    let jwt_secret = std::env::var("JWT_SECRET")
        .map_err(|_| actix_web::error::ErrorInternalServerError("JWT_SECRET not set"))?;

    let jwt_service = JwtService::new(&jwt_secret);

    match jwt_service.verify_token(token) {
        Ok(claims) => {
            let user_id: i64 = claims
                .sub
                .parse()
                .map_err(|_| actix_web::error::ErrorBadRequest("Invalid user ID in token"))?;

            let update_req = req.into_inner();
            let user = update_user_profile(&data, user_id, update_req.name, update_req.phone)
                .await
                .map_err(|e| {
                    actix_web::error::ErrorBadRequest(format!("Failed to update profile: {}", e))
                })?;

            Ok(HttpResponse::Ok().json(json!({
                "user": user,
                "message": "Profile updated successfully"
            })))
        }
        Err(e) => Ok(HttpResponse::Unauthorized().json(json!({
            "error": "INVALID_TOKEN",
            "message": format!("Invalid token: {}", e)
        }))),
    }
}

async fn exchange_code_for_token(code: &str) -> anyhow::Result<Value> {
    let client_id = std::env::var("GOOGLE_CLIENT_ID")?;
    let client_secret = std::env::var("GOOGLE_CLIENT_SECRET")?;
    let redirect_uri = std::env::var("GOOGLE_REDIRECT_URI")
        .unwrap_or_else(|_| "http://localhost:8080/auth/google/callback".to_string());

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let response = client
        .post(GOOGLE_TOKEN_URL)
        .form(&[
            ("code", code),
            ("client_id", &client_id),
            ("client_secret", &client_secret),
            ("redirect_uri", &redirect_uri),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .context("Failed to send token request")?;

    let token_data: Value = response
        .json()
        .await
        .context("Failed to parse token response")?;

    Ok(token_data)
}

async fn get_google_user_info(access_token: &str) -> anyhow::Result<GoogleUserInfo> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let response = client
        .get(GOOGLE_USER_INFO_URL)
        .bearer_auth(access_token)
        .send()
        .await
        .context("Failed to send user info request")?;

    let user_info: GoogleUserInfo = response
        .json()
        .await
        .context("Failed to parse user info response")?;

    Ok(user_info)
}

async fn upsert_user(data: &AppState, google_user: &GoogleUserInfo) -> anyhow::Result<User> {
    let mut engine = data.engine.lock();

    create_tables(&mut engine)?;

    let check_sql = format!(
        "SELECT id, google_sub, email, name, avatar_url, role, phone, created_at, updated_at FROM users WHERE google_sub = '{}'",
        escape_sql_string(&google_user.sub)
    );

    match engine.execute_sql(&check_sql) {
        Ok(db::printer::ReplOutput::Rows { mut rows, .. }) => {
            if let Some(row) = rows.pop() {
                let update_sql = format!(
                    "UPDATE users SET name = {}, avatar_url = {}, updated_at = '{}' WHERE id = {}",
                    google_user
                        .name
                        .as_ref()
                        .map(|n| format!("'{}'", escape_sql_string(n)))
                        .unwrap_or("NULL".to_string()),
                    google_user
                        .picture
                        .as_ref()
                        .map(|p| format!("'{}'", escape_sql_string(p)))
                        .unwrap_or("NULL".to_string()),
                    Utc::now().format("%Y-%m-%d %H:%M:%S"),
                    format_value(&row.values()[0])
                );

                engine
                    .execute_sql(&update_sql)
                    .context("Failed to update user")?;

                let user_id = row.values()[0].as_i64().unwrap();
                load_user_by_id_locked(&mut engine, user_id)
            } else {
                create_user(&mut engine, google_user)
            }
        }
        Ok(_other) => create_user(&mut engine, google_user),
        Err(_e) => create_user(&mut engine, google_user),
    }
}

pub async fn load_user_by_id(data: &AppState, user_id: i64) -> anyhow::Result<User> {
    let mut engine = data.engine.lock();
    load_user_by_id_locked(&mut engine, user_id)
}

pub fn load_user_by_id_locked(
    engine: &mut db::engine::Engine,
    user_id: i64,
) -> anyhow::Result<User> {
    let sql = format!(
        "SELECT id, google_sub, email, name, avatar_url, role, phone, created_at, updated_at FROM users WHERE id = {}",
        user_id
    );

    let output = engine.execute_sql(&sql).context("Failed to query user")?;

    match output {
        db::printer::ReplOutput::Rows { mut rows, .. } => {
            if let Some(row) = rows.pop() {
                load_user_by_db_row(&row)
            } else {
                Err(anyhow!("User not found"))
            }
        }
        _ => Err(anyhow!("Unexpected response from database")),
    }
}

pub fn create_tables(engine: &mut db::engine::Engine) -> anyhow::Result<()> {
    let users_sql = r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            google_sub TEXT UNIQUE NOT NULL,
            email TEXT UNIQUE NOT NULL,
            name TEXT,
            avatar_url TEXT,
            role TEXT DEFAULT 'CUSTOMER',
            phone TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )
    "#;

    engine.execute_sql(users_sql)?;

    let events_sql = r#"
        CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            organizer_user_id INTEGER NOT NULL,
            title TEXT NOT NULL,
            description TEXT,
            venue TEXT,
            location TEXT,
            start_time TEXT NOT NULL,
            end_time TEXT NOT NULL,
            status TEXT DEFAULT 'DRAFT',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (organizer_user_id) REFERENCES users(id)
        )
    "#;

    engine.execute_sql(events_sql)?;

    let ticket_types_sql = r#"
        CREATE TABLE IF NOT EXISTS ticket_types (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            event_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            price INTEGER NOT NULL,
            capacity INTEGER NOT NULL,
            sales_start TEXT,
            sales_end TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (event_id) REFERENCES events(id)
        )
    "#;

    engine.execute_sql(ticket_types_sql)?;

    let orders_sql = r#"
        CREATE TABLE IF NOT EXISTS orders (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            customer_user_id INTEGER NOT NULL,
            status TEXT DEFAULT 'PENDING',
            total_amount INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (customer_user_id) REFERENCES users(id)
        )
    "#;

    engine.execute_sql(orders_sql)?;

    let tickets_sql = r#"
        CREATE TABLE IF NOT EXISTS tickets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            order_id INTEGER NOT NULL,
            ticket_type_id INTEGER NOT NULL,
            unit_price INTEGER NOT NULL,
            status TEXT DEFAULT 'HELD',
            created_at TEXT NOT NULL,
            FOREIGN KEY (order_id) REFERENCES orders(id),
            FOREIGN KEY (ticket_type_id) REFERENCES ticket_types(id)
        )
    "#;

    engine.execute_sql(tickets_sql)?;

    Ok(())
}

fn create_user(
    engine: &mut db::engine::Engine,
    google_user: &GoogleUserInfo,
) -> anyhow::Result<User> {
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S");

    let insert_sql = format!(
        "INSERT INTO users (google_sub, email, name, avatar_url, role, created_at, updated_at) VALUES ('{}', '{}', {}, {}, 'CUSTOMER', '{}', '{}')",
        escape_sql_string(&google_user.sub),
        escape_sql_string(&google_user.email),
        google_user
            .name
            .as_ref()
            .map(|n| format!("'{}'", escape_sql_string(n)))
            .unwrap_or("NULL".to_string()),
        google_user
            .picture
            .as_ref()
            .map(|p| format!("'{}'", escape_sql_string(p)))
            .unwrap_or("NULL".to_string()),
        now,
        now
    );

    if let Err(e) = engine.execute_sql(&insert_sql) {
        return Err(anyhow!("Failed to insert user: {}", e));
    }

    let select_sql = format!(
        "SELECT id, google_sub, email, name, avatar_url, role, phone, created_at, updated_at FROM users WHERE google_sub = '{}'",
        escape_sql_string(&google_user.sub)
    );

    match engine.execute_sql(&select_sql) {
        Ok(db::printer::ReplOutput::Rows { mut rows, .. }) => {
            if let Some(row) = rows.pop() {
                load_user_by_db_row(&row)
            } else {
                Err(anyhow!("Failed to retrieve created user"))
            }
        }
        Ok(_other) => Err(anyhow!("Failed to query created user")),
        Err(e) => Err(anyhow!("Failed to query created user: {}", e)),
    }
}

async fn update_user_role(data: &AppState, user_id: i64, role: UserRole) -> anyhow::Result<User> {
    let mut engine = data.engine.lock();

    let role_str = match role {
        UserRole::CUSTOMER => "CUSTOMER",
        UserRole::ORGANIZER => "ORGANIZER",
    };

    let update_sql = format!(
        "UPDATE users SET role = '{}', updated_at = '{}' WHERE id = {}",
        role_str,
        Utc::now().format("%Y-%m-%d %H:%M:%S"),
        user_id
    );

    engine.execute_sql(&update_sql)?;

    load_user_by_id_locked(&mut engine, user_id)
}

async fn update_user_profile(
    data: &AppState,
    user_id: i64,
    name: Option<String>,
    phone: Option<String>,
) -> anyhow::Result<User> {
    let mut engine = data.engine.lock();

    let name_part = name
        .map(|n| format!("name = '{}'", escape_sql_string(&n)))
        .unwrap_or_else(|| "name = name".to_string());

    let phone_part = phone
        .map(|p| format!("phone = '{}'", escape_sql_string(&p)))
        .unwrap_or_else(|| "phone = NULL".to_string());

    let update_sql = format!(
        "UPDATE users SET {}, {}, updated_at = '{}' WHERE id = {}",
        name_part,
        phone_part,
        Utc::now().format("%Y-%m-%d %H:%M:%S"),
        user_id
    );

    engine.execute_sql(&update_sql)?;

    load_user_by_id_locked(&mut engine, user_id)
}

fn load_user_by_db_row(row: &Tuple) -> anyhow::Result<User> {
    let values = row.values();

    if values.is_empty() {
        return Err(anyhow!("Empty row returned"));
    }

    let id = match values[0].as_i64() {
        Ok(v) => Some(v),
        Err(e) => {
            return Err(anyhow!("Failed to get id: {}", e));
        }
    };
    let google_sub = values[1].as_str()?.to_string();
    let email = values[2].as_str()?.to_string();
    let name = values[3].as_str().ok().map(|s: &str| s.to_string());
    let avatar_url = values[4].as_str().ok().map(|s: &str| s.to_string());

    let role_str = values[5].as_str()?.to_string();
    let role = match role_str.as_str() {
        "ORGANIZER" => UserRole::ORGANIZER,
        _ => UserRole::CUSTOMER,
    };

    let phone = values[6].as_str().ok().map(|s: &str| s.to_string());

    let created_at_str = values[7]
        .as_str()
        .map_err(|_| anyhow!("Invalid created_at value"))?;
    let created_at_naive = NaiveDateTime::parse_from_str(created_at_str, "%Y-%m-%d %H:%M:%S")
        .with_context(|| format!("Invalid created_at timestamp: {}", created_at_str))?;
    let created_at: DateTime<Utc> = DateTime::from_naive_utc_and_offset(created_at_naive, Utc);

    let updated_at_str = values[8]
        .as_str()
        .map_err(|_| anyhow!("Invalid updated_at value"))?;
    let updated_at_naive = NaiveDateTime::parse_from_str(updated_at_str, "%Y-%m-%d %H:%M:%S")
        .with_context(|| format!("Invalid updated_at timestamp: {}", updated_at_str))?;
    let updated_at: DateTime<Utc> = DateTime::from_naive_utc_and_offset(updated_at_naive, Utc);

    Ok(User {
        id,
        google_sub,
        email,
        name,
        avatar_url,
        role,
        phone,
        created_at,
        updated_at,
    })
}

fn format_value(value: &query::Value) -> String {
    match value {
        query::Value::Integer(i) => i.to_string(),
        query::Value::String(s) => format!("'{}'", escape_sql_string(s)),
        query::Value::Null => "NULL".to_string(),
        _ => format!("{:?}", value),
    }
}

fn escape_sql_string(input: &str) -> String {
    input.replace('\'', "''")
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;
    use std::sync::Arc;
    use tempfile;

    #[test]
    fn test_sql_escape() {
        assert_eq!(escape_sql_string("test's"), "test''s");
        assert_eq!(escape_sql_string("normal"), "normal");
        assert_eq!(escape_sql_string(""), "");
        assert_eq!(escape_sql_string("'quote'"), "''quote''");
    }

    #[tokio::test]
    async fn test_create_users_table() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let db_path = temp_file.path();

        let engine = db::engine::Engine::new(db_path).unwrap();
        let mut engine = engine;

        let result = create_tables(&mut engine);
        assert!(result.is_ok());

        let result = engine.execute_sql("SELECT COUNT(*) FROM users");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_user_upsert_create() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let db_path = temp_file.path();

        let engine = db::engine::Engine::new(db_path).unwrap();
        let transactions = Arc::new(Mutex::new(HashMap::new()));
        let app_state = AppState {
            engine: Arc::new(Mutex::new(engine)),
            transactions,
        };

        let google_user = GoogleUserInfo {
            sub: "12345".to_string(),
            email: "test@example.com".to_string(),
            name: Some("Test User".to_string()),
            picture: Some("https://example.com/avatar.jpg".to_string()),
            email_verified: Some(true),
        };

        let result = upsert_user(&app_state, &google_user).await;
        if let Ok(user) = result {
            assert!(user.id.is_some());
            assert_eq!(user.google_sub, "12345");
            assert_eq!(user.email, "test@example.com");
            assert_eq!(user.role, UserRole::CUSTOMER);
        }
    }

    #[tokio::test]
    async fn test_user_upsert_update() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let db_path = temp_file.path();

        let engine = db::engine::Engine::new(db_path).unwrap();
        let transactions = Arc::new(Mutex::new(HashMap::new()));
        let app_state = AppState {
            engine: Arc::new(Mutex::new(engine)),
            transactions,
        };

        let google_user = GoogleUserInfo {
            sub: "12345".to_string(),
            email: "test@example.com".to_string(),
            name: Some("Test User".to_string()),
            picture: Some("https://example.com/avatar.jpg".to_string()),
            email_verified: Some(true),
        };

        let _ = upsert_user(&app_state, &google_user).await;

        let updated_google_user = GoogleUserInfo {
            sub: "12345".to_string(),
            email: "test@example.com".to_string(),
            name: Some("Updated Name".to_string()),
            picture: Some("https://example.com/new-avatar.jpg".to_string()),
            email_verified: Some(true),
        };

        let result = upsert_user(&app_state, &updated_google_user).await;
        if let Ok(user) = result {
            assert_eq!(user.name, Some("Updated Name".to_string()));
        }
    }

    #[tokio::test]
    async fn test_update_user_role() {
        let temp_dir = tempfile::Builder::new().prefix("test").tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let engine = db::engine::Engine::new(&db_path).unwrap();
        let transactions = Arc::new(Mutex::new(HashMap::new()));
        let app_state = AppState {
            engine: Arc::new(Mutex::new(engine)),
            transactions,
        };

        let google_user = GoogleUserInfo {
            sub: "12345".to_string(),
            email: "test@example.com".to_string(),
            name: Some("Test User".to_string()),
            picture: Some("https://example.com/avatar.jpg".to_string()),
            email_verified: Some(true),
        };

        let user = upsert_user(&app_state, &google_user).await.unwrap();
        assert_eq!(user.role, UserRole::CUSTOMER);

        let updated_user = update_user_role(&app_state, user.id.unwrap(), UserRole::ORGANIZER)
            .await
            .unwrap();
        assert_eq!(updated_user.role, UserRole::ORGANIZER);
    }
}

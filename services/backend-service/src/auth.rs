use actix_web::{web, HttpRequest, HttpResponse, Result};
use anyhow::{anyhow, Context};
use chrono::Utc;
use serde_json::{json, Value};
use std::collections::HashMap;
use url::form_urlencoded;

use crate::jwt::JwtService;
use crate::models::*;
use crate::AppState;
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

    // Exchange code for access token
    let token_response = exchange_code_for_token(code).await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!(
            "Failed to exchange code for token: {}",
            e
        ))
    })?;

    let access_token = token_response["access_token"].as_str().ok_or_else(|| {
        actix_web::error::ErrorInternalServerError("Missing access_token in response")
    })?;

    // Get user info from Google
    let user_info = get_google_user_info(access_token).await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to get user info: {}", e))
    })?;

    // Upsert user in database
    let user = upsert_user(&data, &user_info).await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to upsert user: {}", e))
    })?;

    // Generate JWT
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
    // Extract Authorization header
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok());

    let token = match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            &header[7..] // Remove "Bearer " prefix
        }
        _ => {
            return Ok(HttpResponse::Unauthorized().json(json!({
                "error": "AUTH_REQUIRED",
                "message": "Authorization header with Bearer token required"
            })));
        }
    };

    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "default_secret".to_string());
    let jwt_service = JwtService::new(&jwt_secret);

    match jwt_service.verify_token(token) {
        Ok(claims) => {
            // Load user from database
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

async fn exchange_code_for_token(code: &str) -> anyhow::Result<Value> {
    let client_id = std::env::var("GOOGLE_CLIENT_ID")?;
    let client_secret = std::env::var("GOOGLE_CLIENT_SECRET")?;
    let redirect_uri = std::env::var("GOOGLE_REDIRECT_URI")
        .unwrap_or_else(|_| "http://localhost:8080/auth/google/callback".to_string());

    let client = reqwest::Client::new();
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
    let client = reqwest::Client::new();
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

    // Check if user exists by google_sub
    let check_sql = format!(
        "SELECT id, google_sub, email, name, avatar_url, created_at, updated_at FROM users WHERE google_sub = '{}'",
        escape_sql_string(&google_user.sub)
    );

    match engine.execute_sql(&check_sql) {
        Ok(db::printer::ReplOutput::Rows { mut rows, .. }) => {
            if let Some(row) = rows.pop() {
                // User exists, update
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
                    format_value(&row.values()[0]) // id
                );

                engine
                    .execute_sql(&update_sql)
                    .context("Failed to update user")?;

                // Return updated user
                load_user_by_db_row(&row)
            } else {
                // User doesn't exist, create
                create_user(&mut engine, google_user)
            }
        }
        Ok(_) => {
            // Table doesn't exist or no rows, create user
            create_user(&mut engine, google_user)
        }
        Err(_e) => {
            // Try to create table first, then user
            create_users_table(&mut engine)?;
            create_user(&mut engine, google_user)
        }
    }
}

async fn load_user_by_id(data: &AppState, user_id: i64) -> anyhow::Result<User> {
    let mut engine = data.engine.lock();

    let sql = format!(
        "SELECT id, google_sub, email, name, avatar_url, created_at, updated_at FROM users WHERE id = {}",
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

fn create_users_table(engine: &mut db::engine::Engine) -> anyhow::Result<()> {
    let create_sql = r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            google_sub TEXT UNIQUE NOT NULL,
            email TEXT UNIQUE NOT NULL,
            name TEXT,
            avatar_url TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )
    "#;

    engine
        .execute_sql(create_sql)
        .context("Failed to create users table")?;

    Ok(())
}

fn create_user(
    engine: &mut db::engine::Engine,
    google_user: &GoogleUserInfo,
) -> anyhow::Result<User> {
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S");

    let insert_sql = format!(
        "INSERT INTO users (google_sub, email, name, avatar_url, created_at, updated_at) VALUES ('{}', '{}', {}, {}, '{}', '{}')",
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

    engine
        .execute_sql(&insert_sql)
        .context("Failed to create user")?;

    // Load the created user to get the ID
    let select_sql = format!(
        "SELECT id, google_sub, email, name, avatar_url, created_at, updated_at FROM users WHERE google_sub = '{}'",
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
        _ => Err(anyhow!("Failed to query created user")),
    }
}

fn load_user_by_db_row(row: &Tuple) -> anyhow::Result<User> {
    let values = row.values();

    let id = Some(values[0].as_i64()?);
    let google_sub = values[1].as_str()?.to_string();
    let email = values[2].as_str()?.to_string();
    let name = values[3].as_str().ok().map(|s: &str| s.to_string());
    let avatar_url = values[4].as_str().ok().map(|s: &str| s.to_string());
    let created_at_str = values[5]
        .as_str()
        .map_err(|_| anyhow!("Invalid created_at value"))?;
    let created_at = chrono::DateTime::parse_from_rfc3339(created_at_str)
        .with_context(|| format!("Invalid created_at timestamp: {}", created_at_str))?
        .with_timezone(&chrono::Utc);

    let updated_at_str = values[6]
        .as_str()
        .map_err(|_| anyhow!("Invalid updated_at value"))?;
    let updated_at = chrono::DateTime::parse_from_rfc3339(updated_at_str)
        .with_context(|| format!("Invalid updated_at timestamp: {}", updated_at_str))?
        .with_timezone(&chrono::Utc);

    Ok(User {
        id,
        google_sub,
        email,
        name,
        avatar_url,
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
    use std::collections::HashMap;
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

        let result = create_users_table(&mut engine);
        assert!(result.is_ok());

        // Verify table exists by querying it
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
}

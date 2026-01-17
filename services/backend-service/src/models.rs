use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct SqlRequest {
    pub sql: String,
    pub tx_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SqlResponse {
    pub columns: Option<Vec<String>>,
    pub rows: Option<Vec<Vec<SerializableValue>>>,
    pub rows_affected: Option<usize>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error_code: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct TransactionResponse {
    pub tx_id: String,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct SuccessResponse {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub id: Option<i64>,
    pub google_sub: String,
    pub email: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct GoogleUserInfo {
    pub sub: String,
    pub email: String,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub email_verified: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // Internal user ID
    pub email: String,
    pub exp: usize, // Expiration time
    pub iat: usize, // Issued at
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct AuthCallbackRequest {
    pub code: String,
    pub state: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: User,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub user: User,
}

// Re-export SerializableValue from db::printer
pub use db::printer::SerializableValue;

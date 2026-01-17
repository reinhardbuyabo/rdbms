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

// Re-export SerializableValue from db::printer
pub use db::printer::SerializableValue;

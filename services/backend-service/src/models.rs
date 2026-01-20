use anyhow::anyhow;
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum UserRole {
    #[serde(rename = "CUSTOMER")]
    CUSTOMER,
    #[serde(rename = "ORGANIZER")]
    ORGANIZER,
    #[serde(rename = "ADMIN")]
    ADMIN,
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserRole::CUSTOMER => write!(f, "CUSTOMER"),
            UserRole::ORGANIZER => write!(f, "ORGANIZER"),
            UserRole::ADMIN => write!(f, "ADMIN"),
        }
    }
}

impl UserRole {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "ADMIN" => UserRole::ADMIN,
            "ORGANIZER" => UserRole::ORGANIZER,
            "CUSTOMER" => UserRole::CUSTOMER,
            _ => UserRole::CUSTOMER,
        }
    }

    pub fn parse(s: &str) -> anyhow::Result<Self> {
        match s.to_uppercase().as_str() {
            "ADMIN" => Ok(UserRole::ADMIN),
            "ORGANIZER" => Ok(UserRole::ORGANIZER),
            "CUSTOMER" => Ok(UserRole::CUSTOMER),
            _ => Err(anyhow!(
                "Invalid role: {}. Valid roles are: ADMIN, ORGANIZER, CUSTOMER",
                s
            )),
        }
    }

    pub fn can_grant_role(&self, target_role: &UserRole) -> bool {
        match self {
            UserRole::ADMIN => true,
            UserRole::ORGANIZER => *target_role == UserRole::CUSTOMER,
            UserRole::CUSTOMER => false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum EventStatus {
    #[serde(rename = "DRAFT")]
    DRAFT,
    #[serde(rename = "PUBLISHED")]
    PUBLISHED,
    #[serde(rename = "CANCELLED")]
    CANCELLED,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderStatus {
    PENDING,
    PAID,
    CANCELLED,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TicketStatus {
    HELD,
    ISSUED,
    VOID,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub id: Option<i64>,
    pub google_sub: String,
    pub email: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub role: UserRole,
    pub phone: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserUpdateRequest {
    pub name: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RoleChangeRequest {
    pub target_user_id: i64,
    pub role: UserRole,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Event {
    pub id: Option<i64>,
    pub organizer_user_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub venue: Option<String>,
    pub location: Option<String>,
    pub start_time: String,
    pub end_time: String,
    pub status: EventStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticket_types: Option<Vec<TicketType>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_capacity: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_sold: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateEventRequest {
    pub title: String,
    pub description: Option<String>,
    pub venue: Option<String>,
    pub location: Option<String>,
    pub start_time: String,
    pub end_time: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateEventRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub venue: Option<String>,
    pub location: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EventWithTicketTypes {
    pub event: Event,
    pub ticket_types: Vec<TicketTypeWithAvailability>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TicketType {
    pub id: Option<i64>,
    pub event_id: i64,
    pub name: String,
    pub price: i64,
    pub capacity: i64,
    pub sales_start: Option<String>,
    pub sales_end: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TicketTypeWithAvailability {
    pub id: Option<i64>,
    pub event_id: i64,
    pub name: String,
    pub price: i64,
    pub capacity: i64,
    pub sold: i64,
    pub remaining: i64,
    pub sales_start: Option<String>,
    pub sales_end: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTicketTypeRequest {
    pub name: String,
    pub price: i64,
    pub capacity: i64,
    pub sales_start: Option<String>,
    pub sales_end: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateTicketTypeRequest {
    pub name: Option<String>,
    pub price: Option<i64>,
    pub capacity: Option<i64>,
    pub sales_start: Option<String>,
    pub sales_end: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Order {
    pub id: Option<i64>,
    pub customer_user_id: i64,
    pub status: OrderStatus,
    pub total_amount: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderWithDetails {
    pub order: Order,
    pub tickets: Vec<TicketWithDetails>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderItemRequest {
    pub ticket_type_id: i64,
    pub quantity: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateOrderRequest {
    pub items: Vec<OrderItemRequest>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Ticket {
    pub id: Option<i64>,
    pub order_id: i64,
    pub ticket_type_id: i64,
    pub unit_price: i64,
    pub status: TicketStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TicketWithDetails {
    pub id: Option<i64>,
    pub order_id: i64,
    pub ticket_type_id: i64,
    pub unit_price: i64,
    pub status: TicketStatus,
    pub ticket_type_name: String,
    pub ticket_type_price: i64,
    pub event_id: i64,
    pub event_title: String,
    pub event_start_time: String,
    pub event_end_time: String,
    pub event_venue: Option<String>,
    pub event_location: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EventSalesSummary {
    pub event_id: i64,
    pub event_title: String,
    pub total_orders: i64,
    pub total_tickets: i64,
    pub total_revenue: i64,
    pub orders: Vec<OrderWithCustomer>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderWithCustomer {
    pub order: Order,
    pub customer_id: i64,
    pub customer_name: Option<String>,
    pub customer_email: String,
    pub tickets: Vec<TicketWithDetails>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct GoogleUserInfo {
    #[serde(rename = "id")]
    pub sub: String,
    pub email: String,
    pub name: Option<String>,
    pub picture: Option<String>,
    #[serde(rename = "verified_email")]
    pub email_verified: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub role: String,
    pub exp: usize,
    pub iat: usize,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct ListEventsQuery {
    pub status: Option<EventStatus>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub q: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub page: u32,
    pub limit: u32,
    pub total: u64,
    pub total_pages: u32,
}

pub use db::printer::SerializableValue;

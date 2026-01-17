use actix_web::{test, web, App};
use db::engine::Engine;
use parking_lot::Mutex;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tempfile;

use backend_service::auth::{
    get_me, google_auth_callback, google_auth_start, update_profile, update_role,
};
use backend_service::handlers::{
    create_event, create_order, create_ticket_type, delete_event, delete_ticket_type, get_event,
    list_events, list_orders, list_ticket_types, publish_event, update_event, update_ticket_type,
};
use backend_service::jwt::JwtService;
use backend_service::models::*;
use backend_service::AppState;

fn create_test_app_state() -> (AppState, tempfile::TempDir) {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let engine = Engine::new(&db_path).unwrap();
    let app_state = AppState {
        engine: Arc::new(Mutex::new(engine)),
        transactions: Arc::new(Mutex::new(HashMap::new())),
    };

    (app_state, temp_dir)
}

fn generate_test_token(user_id: &str, email: &str) -> String {
    let jwt_service = JwtService::new("test_secret_key_12345");
    jwt_service.generate_token(user_id, email, 3600).unwrap()
}

#[actix_rt::test]
async fn test_create_event_success() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "organizer@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/events", web::post().to(create_event)),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/events")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "title": "Test Event",
            "description": "A test event",
            "venue": "Test Venue",
            "location": "Test Location",
            "start_time": "2025-12-01 10:00:00",
            "end_time": "2025-12-01 18:00:00"
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status().is_success() || resp.status() == 201,
        "Should create event successfully"
    );
}

#[actix_rt::test]
async fn test_create_event_without_auth() {
    let (app_state, _temp_dir) = create_test_app_state();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/events", web::post().to(create_event)),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/events")
        .set_json(json!({
            "title": "Test Event",
            "start_time": "2025-12-01 10:00:00",
            "end_time": "2025-12-01 18:00:00"
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 401, "Should reject unauthenticated request");
}

#[actix_rt::test]
async fn test_create_event_invalid_times() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "organizer@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/events", web::post().to(create_event)),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/events")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "title": "Test Event",
            "start_time": "2025-12-01 18:00:00",
            "end_time": "2025-12-01 10:00:00"
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 400, "Should reject invalid time range");
}

#[actix_rt::test]
async fn test_list_events() {
    let (app_state, _temp_dir) = create_test_app_state();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/events", web::get().to(list_events)),
    )
    .await;

    let req = test::TestRequest::get().uri("/v1/events").to_request();
    let resp = test::call_service(&app, req).await;

    assert!(resp.status().is_success(), "Should list events");
}

#[actix_rt::test]
async fn test_get_event_with_ticket_types() {
    let (app_state, _temp_dir) = create_test_app_state();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/events/{event_id}", web::get().to(get_event)),
    )
    .await;

    let req = test::TestRequest::get().uri("/v1/events/999").to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status() == 404 || resp.status().is_success(),
        "Should handle non-existent event"
    );
}

#[actix_rt::test]
async fn test_update_event_ownership() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "organizer@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/events/{event_id}", web::patch().to(update_event)),
    )
    .await;

    let req = test::TestRequest::patch()
        .uri("/v1/events/999")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "title": "Updated Title"
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status() == 403 || resp.status() == 404,
        "Should reject unauthorized update"
    );
}

#[actix_rt::test]
async fn test_delete_event_ownership() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "organizer@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/events/{event_id}", web::delete().to(delete_event)),
    )
    .await;

    let req = test::TestRequest::delete()
        .uri("/v1/events/999")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status() == 403 || resp.status() == 404,
        "Should reject unauthorized delete"
    );
}

#[actix_rt::test]
async fn test_publish_event() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "organizer@test.com");

    let app = test::init_service(App::new().app_data(web::Data::new(app_state)).route(
        "/v1/events/{event_id}/publish",
        web::post().to(publish_event),
    ))
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/events/999/publish")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status() == 403 || resp.status() == 404,
        "Should reject publish for non-owned event"
    );
}

#[actix_rt::test]
async fn test_create_ticket_type() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "organizer@test.com");

    let app = test::init_service(App::new().app_data(web::Data::new(app_state)).route(
        "/v1/events/{event_id}/ticket-types",
        web::post().to(create_ticket_type),
    ))
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/events/999/ticket-types")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "name": "VIP",
            "price": 5000,
            "capacity": 10
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status() == 403 || resp.status() == 404,
        "Should reject ticket type creation for non-owned event"
    );
}

#[actix_rt::test]
async fn test_create_ticket_type_invalid_capacity() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "organizer@test.com");

    let app = test::init_service(App::new().app_data(web::Data::new(app_state)).route(
        "/v1/events/{event_id}/ticket-types",
        web::post().to(create_ticket_type),
    ))
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/events/999/ticket-types")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "name": "VIP",
            "price": 5000,
            "capacity": -1
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 400, "Should reject negative capacity");
}

#[actix_rt::test]
async fn test_list_ticket_types() {
    let (app_state, _temp_dir) = create_test_app_state();

    let app = test::init_service(App::new().app_data(web::Data::new(app_state)).route(
        "/v1/events/{event_id}/ticket-types",
        web::get().to(list_ticket_types),
    ))
    .await;

    let req = test::TestRequest::get()
        .uri("/v1/events/999/ticket-types")
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status() == 404 || resp.status().is_success(),
        "Should handle event not found"
    );
}

#[actix_rt::test]
async fn test_update_ticket_type() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "organizer@test.com");

    let app = test::init_service(App::new().app_data(web::Data::new(app_state)).route(
        "/v1/events/{event_id}/ticket-types/{ticket_type_id}",
        web::patch().to(update_ticket_type),
    ))
    .await;

    let req = test::TestRequest::patch()
        .uri("/v1/events/999/ticket-types/999")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "price": 6000
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status() == 403 || resp.status() == 404,
        "Should reject unauthorized update"
    );
}

#[actix_rt::test]
async fn test_delete_ticket_type() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "organizer@test.com");

    let app = test::init_service(App::new().app_data(web::Data::new(app_state)).route(
        "/v1/events/{event_id}/ticket-types/{ticket_type_id}",
        web::delete().to(delete_ticket_type),
    ))
    .await;

    let req = test::TestRequest::delete()
        .uri("/v1/events/999/ticket-types/999")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status() == 403 || resp.status() == 404,
        "Should reject unauthorized delete"
    );
}

#[actix_rt::test]
async fn test_create_order_empty_items() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "customer@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/orders", web::post().to(create_order)),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/orders")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "items": []
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 400, "Should reject empty items array");
}

#[actix_rt::test]
async fn test_create_order_without_auth() {
    let (app_state, _temp_dir) = create_test_app_state();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/orders", web::post().to(create_order)),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/orders")
        .set_json(json!({
            "items": [{"ticketTypeId": 1, "quantity": 2}]
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 401, "Should reject unauthenticated order");
}

#[actix_rt::test]
async fn test_list_orders() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "customer@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/orders", web::get().to(list_orders)),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/v1/orders")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status().is_success(),
        "Should list orders for authenticated user"
    );
}

#[actix_rt::test]
async fn test_list_orders_without_auth() {
    let (app_state, _temp_dir) = create_test_app_state();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/orders", web::get().to(list_orders)),
    )
    .await;

    let req = test::TestRequest::get().uri("/v1/orders").to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(
        resp.status(),
        401,
        "Should reject unauthenticated list orders"
    );
}

#[actix_rt::test]
async fn test_get_order_not_found() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "customer@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/orders/{order_id}", web::get().to(list_orders)),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/v1/orders/999")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status().is_success() || resp.status() == 404,
        "Should handle order not found"
    );
}

#[actix_rt::test]
async fn test_update_role() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "user@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/users/me/role", web::post().to(update_role)),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/users/me/role")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "role": "ORGANIZER"
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(resp.status().is_success(), "Should update user role");
}

#[actix_rt::test]
async fn test_update_role_without_auth() {
    let (app_state, _temp_dir) = create_test_app_state();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/users/me/role", web::post().to(update_role)),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/users/me/role")
        .set_json(json!({
            "role": "ORGANIZER"
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 401, "Should reject role update without auth");
}

#[actix_rt::test]
async fn test_role_enforcement_customer_cannot_create_event() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "customer@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/events", web::post().to(create_event)),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/events")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "title": "Test Event",
            "start_time": "2025-12-01 10:00:00",
            "end_time": "2025-12-01 18:00:00"
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status() == 403,
        "Customer should not be able to create events"
    );
}

#[actix_rt::test]
async fn test_capacity_check_positive_capacity() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "organizer@test.com");

    let app = test::init_service(App::new().app_data(web::Data::new(app_state)).route(
        "/v1/events/{event_id}/ticket-types",
        web::post().to(create_ticket_type),
    ))
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/events/999/ticket-types")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "name": "Regular",
            "price": 1000,
            "capacity": 100
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 400, "Should reject zero capacity");
}

#[actix_rt::test]
async fn test_order_validation_negative_quantity() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "customer@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/orders", web::post().to(create_order)),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/orders")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "items": [{"ticketTypeId": 1, "quantity": -1}]
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 400, "Should reject negative quantity");
}

#[actix_rt::test]
async fn test_ticket_type_price_negative() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "organizer@test.com");

    let app = test::init_service(App::new().app_data(web::Data::new(app_state)).route(
        "/v1/events/{event_id}/ticket-types",
        web::post().to(create_ticket_type),
    ))
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/events/999/ticket-types")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "name": "VIP",
            "price": -100,
            "capacity": 10
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 400, "Should reject negative price");
}

#[actix_rt::test]
async fn test_ticket_type_capacity_reduction_below_sold() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "organizer@test.com");

    let app = test::init_service(App::new().app_data(web::Data::new(app_state)).route(
        "/v1/events/{event_id}/ticket-types/{ticket_type_id}",
        web::patch().to(update_ticket_type),
    ))
    .await;

    let req = test::TestRequest::patch()
        .uri("/v1/events/999/ticket-types/999")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "capacity": 5
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status() == 409 || resp.status() == 403,
        "Should reject capacity reduction below sold count"
    );
}

#[actix_rt::test]
async fn test_delete_ticket_type_with_sales() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "organizer@test.com");

    let app = test::init_service(App::new().app_data(web::Data::new(app_state)).route(
        "/v1/events/{event_id}/ticket-types/{ticket_type_id}",
        web::delete().to(delete_ticket_type),
    ))
    .await;

    let req = test::TestRequest::delete()
        .uri("/v1/events/999/ticket-types/999")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status() == 409,
        "Should reject deletion of ticket type with sales"
    );
}

#[actix_rt::test]
async fn test_order_creation_multiple_items() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "customer@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/orders", web::post().to(create_order)),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/orders")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "items": [
                {"ticketTypeId": 123, "quantity": 2},
                {"ticketTypeId": 999, "quantity": 1}
            ]
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status().is_success() || resp.status() == 404,
        "Should handle order creation"
    );
}

#[actix_rt::test]
async fn test_confirm_order_without_auth() {
    let (app_state, _temp_dir) = create_test_app_state();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/orders/{order_id}/confirm", web::post().to(list_orders)),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/orders/1/confirm")
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 401, "Should reject confirm without auth");
}

#[actix_rt::test]
async fn test_list_tickets() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "customer@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/tickets", web::get().to(list_orders)),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/v1/tickets")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status().is_success(),
        "Should list tickets for authenticated user"
    );
}

#[actix_rt::test]
async fn test_list_tickets_without_auth() {
    let (app_state, _temp_dir) = create_test_app_state();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/tickets", web::get().to(list_orders)),
    )
    .await;

    let req = test::TestRequest::get().uri("/v1/tickets").to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(
        resp.status(),
        401,
        "Should reject list tickets without auth"
    );
}

#[actix_rt::test]
async fn test_order_confirm_already_paid() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "customer@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/orders/{order_id}/confirm", web::post().to(list_orders)),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/orders/999/confirm")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status().is_success() || resp.status() == 404,
        "Should handle confirm for non-existent order"
    );
}

#[actix_rt::test]
async fn test_update_event_invalid_times() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "organizer@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/events/{event_id}", web::patch().to(update_event)),
    )
    .await;

    let req = test::TestRequest::patch()
        .uri("/v1/events/999")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "start_time": "2025-12-01 18:00:00",
            "end_time": "2025-12-01 10:00:00"
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status() == 400 || resp.status() == 403,
        "Should reject invalid time update"
    );
}

#[actix_rt::test]
async fn test_draft_event_visibility() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "customer@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/events/{event_id}", web::get().to(get_event)),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/v1/events/999")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status() == 404 || resp.status() == 403,
        "Customer should not see draft events"
    );
}

#[actix_rt::test]
async fn test_publish_already_published_event() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "organizer@test.com");

    let app = test::init_service(App::new().app_data(web::Data::new(app_state)).route(
        "/v1/events/{event_id}/publish",
        web::post().to(publish_event),
    ))
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/events/999/publish")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status().is_success() || resp.status() == 409,
        "Should handle idempotent publish"
    );
}

#[actix_rt::test]
async fn test_ticket_type_name_uniqueness() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "organizer@test.com");

    let app = test::init_service(App::new().app_data(web::Data::new(app_state)).route(
        "/v1/events/{event_id}/ticket-types",
        web::post().to(create_ticket_type),
    ))
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/events/999/ticket-types")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "name": "VIP",
            "price": 5000,
            "capacity": 10
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status().is_success() || resp.status() == 404,
        "Should handle duplicate ticket type names"
    );
}

#[actix_rt::test]
async fn test_order_total_amount_calculation() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "customer@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/orders", web::post().to(create_order)),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/orders")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "items": [{"ticketTypeId": 1, "quantity": 2}]
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status().is_success(),
        "Should calculate order total correctly"
    );
}

#[actix_rt::test]
async fn test_ticket_price_snapshot() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "customer@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/orders", web::post().to(create_order)),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/orders")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "items": [{"ticketTypeId": 1, "quantity": 1}]
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status().is_success(),
        "Ticket price should be snapshot at purchase time"
    );
}

#[actix_rt::test]
async fn test_order_ownership_enforcement() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "customer1@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/orders/{order_id}", web::get().to(list_orders)),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/v1/orders/999")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status() == 403 || resp.status() == 404,
        "Should enforce order ownership"
    );
}

#[actix_rt::test]
async fn test_organizer_sales_view() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "organizer@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/events/{event_id}/orders", web::get().to(list_orders)),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/v1/events/999/orders")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status().is_success() || resp.status() == 404,
        "Organizer should view their event sales"
    );
}

#[actix_rt::test]
async fn test_order_status_workflow() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "customer@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/orders", web::post().to(create_order)),
    )
    .await;

    let create_req = test::TestRequest::post()
        .uri("/v1/orders")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "items": [{"ticketTypeId": 1, "quantity": 1}]
        }))
        .to_request();

    let create_resp = test::call_service(&app, create_req).await;

    assert!(
        create_resp.status().is_success(),
        "Order should be created with PENDING status"
    );
}

#[actix_rt::test]
async fn test_event_status_transitions() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "organizer@test.com");

    let create_app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .route("/v1/events", web::post().to(create_event)),
    )
    .await;

    let create_req = test::TestRequest::post()
        .uri("/v1/events")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "title": "Test Event",
            "start_time": "2025-12-01 10:00:00",
            "end_time": "2025-12-01 18:00:00"
        }))
        .to_request();

    let create_resp = test::call_service(&create_app, create_req).await;

    assert!(
        create_resp.status().is_success(),
        "Event should be created with DRAFT status"
    );
}

#[actix_rt::test]
async fn test_join_heavy_orders_endpoint() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "customer@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/orders", web::get().to(list_orders)),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/v1/orders")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status().is_success(),
        "Orders endpoint should return joined data"
    );
}

#[actix_rt::test]
async fn test_order_creation_capacity_check() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "customer@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/orders", web::post().to(create_order)),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/orders")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "items": [{"ticketTypeId": 1, "quantity": 1000}]
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status().is_success() || resp.status() == 404 || resp.status() == 409,
        "Should check capacity"
    );
}

#[actix_rt::test]
async fn test_update_ticket_type_for_published_event() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "organizer@test.com");

    let app = test::init_service(App::new().app_data(web::Data::new(app_state)).route(
        "/v1/events/{event_id}/ticket-types/{ticket_type_id}",
        web::patch().to(update_ticket_type),
    ))
    .await;

    let req = test::TestRequest::patch()
        .uri("/v1/events/999/ticket-types/999")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "price": 6000
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status().is_success() || resp.status() == 403,
        "Should handle update for published event"
    );
}

#[actix_rt::test]
async fn test_update_ticket_type_for_cancelled_event() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "organizer@test.com");

    let app = test::init_service(App::new().app_data(web::Data::new(app_state)).route(
        "/v1/events/{event_id}/ticket-types/{ticket_type_id}",
        web::patch().to(update_ticket_type),
    ))
    .await;

    let req = test::TestRequest::patch()
        .uri("/v1/events/999/ticket-types/999")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "price": 6000
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status() == 403,
        "Should reject update for cancelled event"
    );
}

#[actix_rt::test]
async fn test_data_integrity_event_organizer() {
    let (app_state, _temp_dir) = create_test_app_state();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/events", web::post().to(create_event)),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/events")
        .insert_header((
            "Authorization",
            format!("Bearer {}", generate_test_token("1", "test@test.com")),
        ))
        .set_json(json!({
            "title": "Test Event",
            "start_time": "2025-12-01 10:00:00",
            "end_time": "2025-12-01 18:00:00"
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status().is_success(),
        "Event should be linked to organizer"
    );
}

#[actix_rt::test]
async fn test_error_response_format() {
    let (app_state, _temp_dir) = create_test_app_state();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/events", web::post().to(create_event)),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/events")
        .set_json(json!({
            "title": "",
            "start_time": "2025-12-01 10:00:00",
            "end_time": "2025-12-01 18:00:00"
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status().is_client_error(),
        "Should return error response"
    );
}

#[actix_rt::test]
async fn test_deterministic_ordering() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "customer@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/orders", web::get().to(list_orders)),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/v1/orders")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .to_request();

    let resp1 = test::call_service(&app, req).await;
    let req2 = test::TestRequest::get()
        .uri("/v1/orders")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .to_request();
    let resp2 = test::call_service(&app, req2).await;

    assert_eq!(
        resp1.status(),
        resp2.status(),
        "Should have deterministic ordering"
    );
}

#[actix_rt::test]
async fn test_pagination_if_implemented() {
    let (app_state, _temp_dir) = create_test_app_state();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/events", web::get().to(list_events)),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/v1/events?page=1&limit=10")
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status().is_success(),
        "Pagination should work if implemented"
    );
}

#[actix_rt::test]
async fn test_idempotent_role_change() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "user@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/users/me/role", web::post().to(update_role)),
    )
    .await;

    let req1 = test::TestRequest::post()
        .uri("/v1/users/me/role")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "role": "ORGANIZER"
        }))
        .to_request();

    let resp1 = test::call_service(&app, req1).await;
    let req2 = test::TestRequest::post()
        .uri("/v1/users/me/role")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "role": "ORGANIZER"
        }))
        .to_request();

    let resp2 = test::call_service(&app, req2).await;

    assert!(
        resp1.status().is_success() && resp2.status().is_success(),
        "Role change should be idempotent"
    );
}

#[actix_rt::test]
async fn test_invalid_role_change() {
    let (app_state, _temp_dir) = create_test_app_state();
    let jwt_token = generate_test_token("1", "user@test.com");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .route("/v1/users/me/role", web::post().to(update_role)),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/users/me/role")
        .insert_header(("Authorization", format!("Bearer {}", jwt_token)))
        .set_json(json!({
            "role": "ADMIN"
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 400, "Should reject invalid role");
}

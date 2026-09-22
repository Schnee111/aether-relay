use aether_relay::config::AppConfig;
use aether_relay::db::create_pool;
use aether_relay::state::AppState;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

fn setup_app() -> axum::Router {
    let pool = create_pool(":memory:", 1, 5000, 0, -2000).unwrap();
    let config = AppConfig::load().unwrap();
    let state = AppState { pool, config };
    aether_relay::api::create_router(state)
}

#[tokio::test]
async fn test_endpoint_crud_lifecycle() {
    let app = setup_app();

    // 1. Create endpoint -> 201 Created
    let payload = json!({
        "name": "GitHub Prod",
        "provider": "github",
        "secret": "gh_secret_123",
        "target_url": "https://downstream.example.com/hook"
    });

    let create_req = Request::builder()
        .uri("/v1/endpoints")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let create_resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(create_resp.status(), StatusCode::CREATED);

    let create_bytes = create_resp.into_body().collect().await.unwrap().to_bytes();
    let create_json: serde_json::Value = serde_json::from_slice(&create_bytes).unwrap();
    let endpoint_id = create_json["id"].as_str().unwrap().to_string();

    // 2. List endpoints -> 200 OK with 1 item
    let list_req = Request::builder()
        .uri("/v1/endpoints")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let list_resp = app.clone().oneshot(list_req).await.unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);

    let list_bytes = list_resp.into_body().collect().await.unwrap().to_bytes();
    let list_json: serde_json::Value = serde_json::from_slice(&list_bytes).unwrap();
    assert_eq!(list_json.as_array().unwrap().len(), 1);
    assert_eq!(list_json[0]["id"], endpoint_id);

    // 3. Delete endpoint -> 204 No Content
    let delete_req = Request::builder()
        .uri(format!("/v1/endpoints/{}", endpoint_id))
        .method("DELETE")
        .body(Body::empty())
        .unwrap();

    let delete_resp = app.clone().oneshot(delete_req).await.unwrap();
    assert_eq!(delete_resp.status(), StatusCode::NO_CONTENT);

    // 4. List again -> empty
    let list_req2 = Request::builder()
        .uri("/v1/endpoints")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let list_resp2 = app.oneshot(list_req2).await.unwrap();
    let list_bytes2 = list_resp2.into_body().collect().await.unwrap().to_bytes();
    let list_json2: serde_json::Value = serde_json::from_slice(&list_bytes2).unwrap();
    assert_eq!(list_json2.as_array().unwrap().len(), 0);
}

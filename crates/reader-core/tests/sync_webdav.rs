//! CAP-SYNC: WebDAV sync exercised against a local axum mock server.
//! No external network is required; the mock implements the subset used by
//! ReaderCore: PROPFIND, MKCOL, PUT and GET.

use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{Method, Request, Response, StatusCode},
    routing::any,
    Router,
};
use reader_core::{AddBookPayload, ReaderCore, ReaderCoreOptions};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::Mutex;

#[derive(Default)]
struct MockDavState {
    collections: HashSet<String>,
    files: HashMap<String, Value>,
}

async fn webdav(
    State(state): State<Arc<Mutex<MockDavState>>>,
    req: Request<Body>,
) -> Response<Body> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let mut guard = state.lock().await;

    if method.as_str() == "PROPFIND" {
        let exists = guard.collections.contains(&path) || guard.files.contains_key(&path);
        return response(if exists {
            StatusCode::from_u16(207).unwrap()
        } else {
            StatusCode::NOT_FOUND
        });
    }
    if method.as_str() == "MKCOL" {
        guard.collections.insert(path);
        return response(StatusCode::CREATED);
    }
    if method == Method::PUT {
        let body = to_bytes(req.into_body(), usize::MAX).await.unwrap();
        let value: Value = serde_json::from_slice(&body).unwrap();
        guard.files.insert(path, value);
        return response(StatusCode::CREATED);
    }
    if method == Method::GET {
        if let Some(value) = guard.files.get(&path) {
            return Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(value).unwrap()))
                .unwrap();
        }
        return response(StatusCode::NOT_FOUND);
    }
    response(StatusCode::METHOD_NOT_ALLOWED)
}

fn response(status: StatusCode) -> Response<Body> {
    Response::builder()
        .status(status)
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn webdav_sync_push_pull_round_trip() {
    let state = Arc::new(Mutex::new(MockDavState::default()));
    let app = Router::new()
        .route("/dav/*path", any(webdav))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let temp = tempfile::tempdir().unwrap();
    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .unwrap();
    core.app_config_set("sync_enabled", &json!(true))
        .await
        .unwrap();
    core.app_config_set("sync_webdav_url", &json!(format!("http://{addr}/dav")))
        .await
        .unwrap();
    core.app_config_set("sync_webdav_allow_http", &json!(true))
        .await
        .unwrap();
    core.app_config_set("sync_webdav_root_dir", &json!("legado-sync"))
        .await
        .unwrap();
    core.sync_set_credentials("secret").await.unwrap();
    let credentials = core.sync_get_credentials().await.unwrap();
    assert_eq!(credentials.password, "");
    assert!(credentials.password_set);

    let connection = core.sync_test_connection(None).await.unwrap();
    assert!(connection.ok, "{}", connection.message);

    let book = core
        .shelf_add(
            AddBookPayload {
                name: "Demo Book".to_string(),
                author: Some("Tester".to_string()),
                cover_url: None,
                intro: Some("intro".to_string()),
                kind: Some("test".to_string()),
                group_id: None,
                book_url: "https://example.test/book".to_string(),
                source_dir: None,
                last_chapter: Some("Chapter 1".to_string()),
                source_type: Some("novel".to_string()),
            },
            "demo.js",
            "Demo Source",
        )
        .await
        .unwrap();
    core.sync_client_state_set("reader_settings", json!({"fontSize": 18}))
        .await
        .unwrap();

    let pushed = core
        .sync_now(
            "push",
            Some(vec!["bookshelf".to_string(), "reader_settings".to_string()]),
            None,
        )
        .await
        .unwrap();
    assert_eq!(pushed.status, "success");
    assert_eq!(pushed.uploaded_domains.len(), 2);

    core.shelf_remove(&book.id).await.unwrap();
    core.sync_client_state_set("reader_settings", Value::Null)
        .await
        .unwrap();

    let pulled = core
        .sync_now(
            "pull",
            Some(vec!["bookshelf".to_string(), "reader_settings".to_string()]),
            None,
        )
        .await
        .unwrap();
    assert_eq!(pulled.status, "success");
    assert_eq!(pulled.applied_domains.len(), 2);
    assert_eq!(pulled.client_states.len(), 1);
    assert_eq!(pulled.client_states[0].domain, "reader_settings");
    assert_eq!(pulled.client_states[0].value["fontSize"], 18);

    let shelf = core.shelf_list().await.unwrap();
    assert_eq!(shelf.len(), 1);
    assert_eq!(shelf[0].name, "Demo Book");
}

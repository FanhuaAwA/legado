//! R-P2-008 试点：WS 命令路由与协议处理的集成测试。
//!
//! 必须是集成测试而非 lib 单元测试：tauri/test 的 mock runtime 链入
//! TaskDialogIndirect（Common Controls v6），build.rs 通过
//! `cargo:rustc-link-arg-tests` 只为测试目标注入 v6 manifest，
//! 该指令仅对 tests/ 目标生效。

use legado_tauri_lib::commands::router;
use legado_tauri_lib::state::{AppState, TaskRegistry};
use legado_tauri_lib::ws_server;
use reader_core::{AddBookPayload, ReaderCore, ReaderCoreOptions, SecureMode};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tauri::{Listener, Manager};

async fn test_app() -> (tauri::App<tauri::test::MockRuntime>, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let core = ReaderCore::new(ReaderCoreOptions {
        app_data_dir: dir.path().to_path_buf(),
        request_timeout_secs: 5,
        user_agent: None,
        secure_mode: SecureMode::Normal,
    })
    .await
    .expect("ReaderCore::new");
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("mock app");
    app.manage(AppState {
        core: Arc::new(core),
        tasks: TaskRegistry::default(),
    });
    (app, dir)
}

// ── router::dispatch ───────────────────────────────────────────────────────

#[tokio::test]
async fn unknown_command_is_rejected() {
    let (app, _dir) = test_app().await;
    let err = router::dispatch(app.handle(), "open_dir_in_explorer", &json!({}))
        .await
        .expect_err("未入白名单的命令必须被拒绝");
    assert!(err.starts_with("NOT_ROUTED"), "err = {err}");
}

#[tokio::test]
async fn js_eval_is_security_blocked() {
    let (app, _dir) = test_app().await;
    let err = router::dispatch(app.handle(), "js_eval", &json!({"code": "1+1"}))
        .await
        .expect_err("js_eval 必须被阻断");
    assert!(err.starts_with("SECURITY_BLOCKED"), "err = {err}");
}

#[tokio::test]
async fn invalid_args_are_rejected() {
    let (app, _dir) = test_app().await;
    let err = router::dispatch(app.handle(), "config_read", &json!({"scope": "s"}))
        .await
        .expect_err("缺少必填参数必须报错");
    assert!(err.starts_with("INVALID_ARGS"), "err = {err}");
}

#[tokio::test]
async fn get_platform_returns_string() {
    let (app, _dir) = test_app().await;
    let value = router::dispatch(app.handle(), "get_platform", &json!({}))
        .await
        .expect("get_platform 应成功");
    assert!(value.is_string());
}

#[tokio::test]
async fn config_roundtrip_through_dispatch() {
    let (app, _dir) = test_app().await;
    router::dispatch(
        app.handle(),
        "config_write",
        &json!({"scope": "test", "key": "k1", "value": "v1"}),
    )
    .await
    .expect("config_write 应成功");
    let value = router::dispatch(
        app.handle(),
        "config_read",
        &json!({"scope": "test", "key": "k1"}),
    )
    .await
    .expect("config_read 应成功");
    assert_eq!(value, json!("v1"));
}

#[tokio::test]
async fn capabilities_get_returns_map() {
    let (app, _dir) = test_app().await;
    let value = router::dispatch(app.handle(), "capabilities_get", &json!({}))
        .await
        .expect("capabilities_get 应成功");
    assert!(value.is_object());
    assert!(value.get("sync").is_some());
    assert_eq!(value["syncWebdav"]["supported"], true);
    assert_eq!(value["coverCache"]["supported"], true);
}

#[tokio::test]
async fn repository_commands_are_routed() {
    let (app, _dir) = test_app().await;
    // booksource_check_update reaches the facade (missing source -> command
    // error), proving it is on the form-B whitelist rather than NOT_ROUTED.
    let err = router::dispatch(
        app.handle(),
        "booksource_check_update",
        &json!({"fileName": "does-not-exist.js"}),
    )
    .await
    .expect_err("缺失书源应报命令错误");
    assert!(!err.starts_with("NOT_ROUTED"), "应已路由，实际: {err}");

    // repository_fetch missing its required `url` -> INVALID_ARGS (routed+parsed).
    let err = router::dispatch(app.handle(), "repository_fetch", &json!({}))
        .await
        .expect_err("缺少 url 应报错");
    assert!(err.starts_with("INVALID_ARGS"), "实际: {err}");

    let err = router::dispatch(
        app.handle(),
        "repository_install",
        &json!({
            "downloadUrl": "not-a-url",
            "fileName": "demo.js",
            "expectedUuid": null,
            "sourceDir": "C:/not-real-source-dir"
        }),
    )
    .await
    .expect_err("invalid URL should reach repository_install command logic");
    assert!(
        !err.starts_with("INVALID_ARGS"),
        "sourceDir should parse before command logic runs: {err}"
    );

    let err = router::dispatch(
        app.handle(),
        "repository_check_source_sync",
        &json!({
            "fileName": "missing.js",
            "downloadUrl": "http://127.0.0.1/source.js",
            "expectedUuid": null,
            "sourceDir": "C:/not-real-source-dir"
        }),
    )
    .await
    .expect_err("missing local source should reach repository_check_source_sync command logic");
    assert!(
        !err.starts_with("INVALID_ARGS"),
        "sourceDir should parse before command logic runs: {err}"
    );
}

#[tokio::test]
async fn booksource_list_streaming_is_routed() {
    let (app, _dir) = test_app().await;
    let value = router::dispatch(
        app.handle(),
        "booksource_list_streaming",
        &json!({"requestId": "stream-test", "force": true}),
    )
    .await
    .expect("booksource_list_streaming 应进入 WS 路由");
    assert_eq!(value, Value::Null);
}

#[tokio::test]
async fn bookshelf_prefetch_accepts_direct_payload_and_emits_done() {
    let (app, _dir) = test_app().await;
    let state = app.state::<AppState>();
    let book = state
        .core
        .shelf_add(
            AddBookPayload {
                name: "Prefetch Router Book".to_string(),
                author: Some("Tester".to_string()),
                cover_url: None,
                intro: None,
                kind: None,
                group_id: None,
                book_url: "fixture://prefetch-router/book".to_string(),
                source_dir: None,
                last_chapter: None,
                source_type: Some("novel".to_string()),
            },
            "missing.js",
            "Prefetch Router Source",
        )
        .await
        .unwrap();
    state
        .core
        .shelf_save_chapters(&book.id, Vec::new())
        .await
        .unwrap();

    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let _event_id = app.listen("shelf:prefetch-done", move |event| {
        let _ = tx.send(event.payload().to_string());
    });
    let value = router::dispatch(
        app.handle(),
        "bookshelf_prefetch_chapters",
        &json!({
            "id": book.id,
            "fileName": "missing.js",
            "sourceDir": null,
            "taskId": "prefetch-router-direct",
            "startIndex": 0,
            "count": 0
        }),
    )
    .await
    .expect("direct prefetch payload should route and parse");
    assert_eq!(value, json!(0));

    let event = rx
        .recv_timeout(Duration::from_secs(2))
        .expect("prefetch done event should be emitted");
    let event: Value = serde_json::from_str(&event).expect("event json");
    assert_eq!(event["taskId"], "prefetch-router-direct");
    assert!(event["error"].is_null());
}

#[tokio::test]
async fn booksource_import_legacy_json_text_accepts_request_id_in_ws_router() {
    let (app, _dir) = test_app().await;
    let content = json!({
        "bookSourceName": "WS Import Progress Fixture",
        "bookSourceUrl": "https://ws-import-progress.example/source",
        "enabled": true,
        "ruleSearch": {
            "bookList": "$[*]",
            "name": "name",
            "author": "author",
            "bookUrl": "url"
        }
    })
    .to_string();
    let value = router::dispatch(
        app.handle(),
        "booksource_import_legacy_json_text",
        &json!({
            "content": content,
            "smartExploreSubCategories": false,
            "requestId": "ws-import-progress-test"
        }),
    )
    .await
    .expect("booksource_import_legacy_json_text 应接受 requestId 并进入 WS 路由");
    assert_eq!(value["imported"], 1);
}

#[tokio::test]
async fn booksource_import_legacy_json_texts_accepts_request_id_in_ws_router() {
    let (app, _dir) = test_app().await;
    let items = (0..2)
        .map(|index| {
            json!({
                "label": format!("batch-{index}.json"),
                "content": json!({
                    "bookSourceName": format!("WS Batch Import Fixture {index}"),
                    "bookSourceUrl": format!("https://ws-batch-import.example/source/{index}"),
                    "enabled": true,
                    "ruleSearch": {
                        "bookList": "$[*]",
                        "name": "name",
                        "author": "author",
                        "bookUrl": "url"
                    }
                })
                .to_string()
            })
        })
        .collect::<Vec<_>>();
    let value = router::dispatch(
        app.handle(),
        "booksource_import_legacy_json_texts",
        &json!({
            "items": items,
            "smartExploreSubCategories": false,
            "requestId": "ws-import-batch-progress-test"
        }),
    )
    .await
    .expect("booksource_import_legacy_json_texts 应接受 requestId 并进入 WS 路由");
    assert_eq!(value["imported"], 2);
}

#[tokio::test]
async fn booksource_search_accepts_task_id_in_ws_router() {
    let (app, _dir) = test_app().await;
    let err = router::dispatch(
        app.handle(),
        "booksource_search",
        &json!({
            "fileName": "missing.legado.json",
            "keyword": "测试",
            "page": 1,
            "taskId": "search-router-test",
            "sourceDir": null
        }),
    )
    .await
    .expect_err("缺失书源应报命令错误");
    assert!(
        !err.starts_with("INVALID_ARGS"),
        "taskId 应可被解析，实际: {err}"
    );
    assert!(!err.starts_with("NOT_ROUTED"), "应已路由，实际: {err}");
}

#[tokio::test]
async fn webdav_sync_commands_are_routed() {
    let (app, _dir) = test_app().await;
    let value = router::dispatch(app.handle(), "sync_get_status", &json!({}))
        .await
        .expect("sync_get_status 应成功");
    assert!(value.is_object());
    assert_eq!(value["running"], false);

    let err = router::dispatch(app.handle(), "sync_now", &json!({}))
        .await
        .expect_err("缺少 mode 应报参数错误");
    assert!(err.starts_with("INVALID_ARGS"), "实际: {err}");
}

#[tokio::test]
async fn ai_http_proxy_command_is_routed_and_blocks_local_targets() {
    let (app, _dir) = test_app().await;
    let err = router::dispatch(
        app.handle(),
        "ai_http_proxy_request",
        &json!({
            "request": {
                "url": "http://127.0.0.1/v1/chat/completions",
                "method": "POST",
                "body": "{}",
                "headers": ["content-type: application/json"]
            }
        }),
    )
    .await
    .expect_err("内网目标应被 AI 代理拒绝");
    assert!(!err.starts_with("NOT_ROUTED"), "应已路由，实际: {err}");
    assert!(err.contains("blocked"), "实际: {err}");
}

// ── ws_server::handle_invoke 协议层 ────────────────────────────────────────

#[tokio::test]
async fn invoke_message_produces_response_with_same_id() {
    let (app, _dir) = test_app().await;
    let raw = r#"{"type":"invoke","id":"req-1","cmd":"get_platform","args":{}}"#;
    let response = ws_server::handle_invoke(app.handle(), raw)
        .await
        .expect("应有响应");
    let value: Value = serde_json::from_str(&response).expect("响应应为 JSON");
    assert_eq!(value["type"], "response");
    assert_eq!(value["id"], "req-1");
    assert!(value["data"].is_string());
    assert!(value.get("error").is_none());
}

#[tokio::test]
async fn not_routed_command_produces_error_response() {
    let (app, _dir) = test_app().await;
    let raw = r#"{"type":"invoke","id":"req-2","cmd":"open_dir_in_explorer","args":{}}"#;
    let response = ws_server::handle_invoke(app.handle(), raw)
        .await
        .expect("应有响应");
    let value: Value = serde_json::from_str(&response).expect("响应应为 JSON");
    assert_eq!(value["id"], "req-2");
    assert!(value["error"]
        .as_str()
        .unwrap_or("")
        .starts_with("NOT_ROUTED"));
}

#[tokio::test]
async fn non_invoke_message_is_ignored() {
    let (app, _dir) = test_app().await;
    let raw = r#"{"type":"ping","id":"x"}"#;
    assert!(ws_server::handle_invoke(app.handle(), raw).await.is_none());
}

#[tokio::test]
async fn cover_cache_commands_are_routed() {
    let (app, _dir) = test_app().await;
    let size = router::dispatch(app.handle(), "cover_cache_size", &json!({}))
        .await
        .expect("cover_cache_size should be routed");
    assert_eq!(size, json!(0));

    let cleared = router::dispatch(app.handle(), "cover_cache_clear", &json!({}))
        .await
        .expect("cover_cache_clear should be routed");
    assert_eq!(cleared, json!(0));

    let err = router::dispatch(
        app.handle(),
        "cover_resolve_cache",
        &json!({
            "request": {
                "url": "file:///tmp/not-allowed.png",
                "referer": null,
                "headers": null
            }
        }),
    )
    .await
    .expect_err("unsupported scheme should return a command error, not a route error");
    assert!(!err.starts_with("NOT_ROUTED"), "should be routed: {err}");
    assert!(!err.starts_with("INVALID_ARGS"), "args should parse: {err}");
}

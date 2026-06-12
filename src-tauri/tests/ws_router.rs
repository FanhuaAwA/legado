//! R-P2-008 试点：WS 命令路由与协议处理的集成测试。
//!
//! 必须是集成测试而非 lib 单元测试：tauri/test 的 mock runtime 链入
//! TaskDialogIndirect（Common Controls v6），build.rs 通过
//! `cargo:rustc-link-arg-tests` 只为测试目标注入 v6 manifest，
//! 该指令仅对 tests/ 目标生效。

use legado_tauri_lib::commands::router;
use legado_tauri_lib::state::{AppState, TaskRegistry};
use legado_tauri_lib::ws_server;
use reader_core::{ReaderCore, ReaderCoreOptions, SecureMode};
use serde_json::{json, Value};
use std::sync::Arc;
use tauri::Manager;

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

//! CAP-REPO: book-source repository + `@updateUrl` auto-update, exercised
//! end-to-end against a local axum mock server (no external network).

use axum::{routing::get, Router};
use reader_core::{ReaderCore, ReaderCoreOptions};

const SOURCE_V2: &str = "// @name        Demo Source\n\
// @version     2.0.0\n\
// @uuid        abc-123\n\
// @url         https://demo.example\n\
// @author      Tester\n\
// @enabled     true\n\
function search() { return []; }\n";

const REPO_MANIFEST: &str = r#"{
  "name": "Test Repo",
  "version": "1.0",
  "updatedAt": "2026-06-12",
  "sources": [
    { "uuid": "abc-123", "name": "Demo Source", "version": "2.0.0",
      "fileName": "demo.js", "downloadUrl": "/source.js", "enabled": true }
  ]
}"#;

async fn manifest() -> &'static str {
    REPO_MANIFEST
}
async fn source_v2() -> &'static str {
    SOURCE_V2
}

#[tokio::test]
async fn repository_and_source_update_round_trip() {
    let app = Router::new()
        .route("/repo.json", get(manifest))
        .route("/source.js", get(source_v2));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{addr}");

    let temp = tempfile::tempdir().unwrap();
    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .unwrap();

    // 1) fetch manifest
    let manifest = core
        .repository_fetch(&format!("{base}/repo.json"))
        .await
        .expect("repository_fetch should parse manifest");
    assert_eq!(manifest.name, "Test Repo");
    assert_eq!(manifest.sources.len(), 1);
    assert_eq!(manifest.sources[0].file_name, "demo.js");
    assert_eq!(manifest.sources[0].download_url, "/source.js");

    let source_url = format!("{base}/source.js");

    // 2) preview with matching uuid
    let preview = core
        .repository_preview_source(&source_url, Some("abc-123"))
        .await
        .expect("preview should succeed with matching uuid");
    assert_eq!(preview.meta.name, "Demo Source");
    assert!(preview.has_explicit_uuid);

    // 3) preview with wrong uuid must fail
    assert!(
        core.repository_preview_source(&source_url, Some("wrong"))
            .await
            .is_err(),
        "preview must reject a UUID mismatch"
    );

    // 4) install, then the file is on disk with the remote version
    core.repository_install(&source_url, "demo.js", Some("abc-123"), None)
        .await
        .expect("install should succeed");
    let installed = core.read_source("demo.js", None).await.unwrap();
    assert!(installed.contains("@version     2.0.0"));

    // install must reject a non-.js file name and non-source content
    assert!(core
        .repository_install(&source_url, "demo.txt", None, None)
        .await
        .is_err());
    assert!(
        core.repository_install(&format!("{base}/repo.json"), "x.js", None, None)
            .await
            .is_err(),
        "installing a JSON manifest as a source must fail"
    );

    // 5) consistency check: freshly installed copy matches the remote
    let sync = core
        .repository_check_source_sync("demo.js", &source_url, Some("abc-123"), None)
        .await
        .expect("sync check should succeed");
    assert!(sync.is_consistent);
    assert_eq!(sync.local_version, "2.0.0");
    assert_eq!(sync.remote_version, "2.0.0");
    assert_eq!(sync.uuid, "abc-123");

    // 5b) consistency check and install must respect external source dirs.
    let external = tempfile::tempdir().unwrap();
    let external_dir = external.path().to_str().unwrap();
    core.add_source_dir(external_dir).await.unwrap();
    let external_v1 = format!(
        "// @name        Demo Source\n\
// @version     1.0.0\n\
// @uuid        abc-123\n\
// @url         https://demo.example\n\
// @updateUrl   {source_url}\n\
// @enabled     false\n\
function search() {{ return []; }}\n"
    );
    core.save_js_source("external-repo.js", &external_v1, Some(external_dir))
        .await
        .unwrap();

    let external_sync = core
        .repository_check_source_sync(
            "external-repo.js",
            &source_url,
            Some("abc-123"),
            Some(external_dir),
        )
        .await
        .expect("external sync check should read from the external source dir");
    assert!(!external_sync.is_consistent);
    assert_eq!(external_sync.local_version, "1.0.0");
    assert_eq!(external_sync.remote_version, "2.0.0");

    core.repository_install(
        &source_url,
        "external-repo.js",
        Some("abc-123"),
        Some(external_dir),
    )
    .await
    .expect("install should overwrite the external source file");
    let external_updated = core
        .read_source("external-repo.js", Some(external_dir))
        .await
        .unwrap();
    assert!(external_updated.contains("@version     2.0.0"));
    assert!(
        core.read_source("external-repo.js", None).await.is_err(),
        "repository install with source_dir must not create a default-dir duplicate"
    );

    // 6) @updateUrl auto-update: a local v1 source pointing at the v2 endpoint
    let v1 = format!(
        "// @name        Demo Source\n\
// @version     1.0.0\n\
// @uuid        abc-123\n\
// @url         https://demo.example\n\
// @updateUrl   {source_url}\n\
// @enabled     false\n\
function search() {{ return []; }}\n"
    );
    core.save_js_source("upd.js", &v1, None).await.unwrap();

    let check = core
        .check_source_update("upd.js", None)
        .await
        .expect("check_source_update should succeed");
    assert!(check.has_update, "v1 -> v2 should report an update");
    assert_eq!(check.local_version, "1.0.0");
    assert_eq!(check.remote_version, "2.0.0");

    core.apply_source_update("upd.js", None)
        .await
        .expect("apply_source_update should succeed");
    let updated = core.read_source("upd.js", None).await.unwrap();
    assert!(updated.contains("@version     2.0.0"), "version bumped");
    // local @enabled=false must be preserved across the update
    assert!(
        updated.contains("@enabled     false") || updated.contains("@enabled false"),
        "disabled state must carry over: {updated}"
    );
}

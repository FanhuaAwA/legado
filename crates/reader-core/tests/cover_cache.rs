use axum::http::header;
use axum::routing::get;
use axum::Router;
use futures::future::join_all;
use reader_core::{ReaderCore, ReaderCoreOptions};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn cover_cache_downloads_reuses_and_clears() {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_for_route = hits.clone();
    let app = Router::new().route(
        "/cover.png",
        get(move || {
            let hits = hits_for_route.clone();
            async move {
                hits.fetch_add(1, Ordering::SeqCst);
                ([(header::CONTENT_TYPE, "image/png")], "cover-bytes")
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let temp = tempfile::tempdir().unwrap();
    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .unwrap();
    let url = format!("http://{addr}/cover.png");

    let first = core
        .resolve_cover_cache(&url, Some("http://referer.example/book"), None)
        .await
        .unwrap();
    assert!(first.ends_with(".png"), "path = {first}");
    assert!(std::path::Path::new(&first).exists());
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert!(core.cover_cache_size().await.unwrap() >= "cover-bytes".len() as u64);

    let second = core.resolve_cover_cache(&url, None, None).await.unwrap();
    assert_eq!(second, first);
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    let freed = core.clear_cover_cache().await.unwrap();
    assert!(freed >= "cover-bytes".len() as u64);
    assert_eq!(core.cover_cache_size().await.unwrap(), 0);
}

#[tokio::test]
async fn cover_cache_coalesces_concurrent_requests_for_same_url() {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_for_route = hits.clone();
    let app = Router::new().route(
        "/slow-cover.png",
        get(move || {
            let hits = hits_for_route.clone();
            async move {
                hits.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(75)).await;
                ([(header::CONTENT_TYPE, "image/png")], "cover-bytes")
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let temp = tempfile::tempdir().unwrap();
    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .unwrap();
    let url = format!("http://{addr}/slow-cover.png");
    let tasks = (0..12).map(|_| {
        let core = core.clone();
        let url = url.clone();
        tokio::spawn(async move { core.resolve_cover_cache(&url, None, None).await.unwrap() })
    });

    let paths = join_all(tasks)
        .await
        .into_iter()
        .map(|result| result.unwrap())
        .collect::<Vec<_>>();
    assert!(paths.iter().all(|path| path == &paths[0]));
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

use axum::{response::Html, routing::get, Router};
use reader_core::{AddBookPayload, CachedChapter, ReaderCore, ReaderCoreOptions};
use serde_json::json;
use std::sync::{Arc, Mutex};

async fn search() -> Html<&'static str> {
    Html(
        r#"
        <html><body>
          <div class="item">
            <a class="title" href="/book/1">Route B Book</a>
            <span class="author">Codex</span>
            <span class="intro">A native reader-core fixture.</span>
          </div>
        </body></html>
        "#,
    )
}

async fn book() -> Html<&'static str> {
    Html(
        r#"
        <html><body>
          <h1>Route B Book</h1>
          <div class="author">Codex</div>
          <a id="toc" href="/book/1/toc">目录</a>
          <div id="intro">A native reader-core fixture.</div>
        </body></html>
        "#,
    )
}

async fn toc() -> Html<&'static str> {
    Html(
        r#"
        <html><body>
          <ul id="chapters">
            <li><a href="/chapter/1">第一章 原生路线</a></li>
          </ul>
        </body></html>
        "#,
    )
}

async fn content() -> Html<&'static str> {
    Html(
        r#"
        <html><body>
          <article id="content">
            <p>这是 Route B 的第一章正文。</p>
          </article>
        </body></html>
        "#,
    )
}

async fn prefetch_toc() -> Html<&'static str> {
    Html(
        r#"
        <html><body>
          <ul id="chapters">
            <li><a href="/prefetch/chapter/1">第一章 已缓存</a></li>
            <li><a href="/prefetch/chapter/2">第二章 待缓存</a></li>
          </ul>
        </body></html>
        "#,
    )
}

async fn prefetch_content_one() -> Html<&'static str> {
    Html(
        r#"
        <html><body>
          <article id="content">
            <p>第一章预取测试正文。</p>
          </article>
        </body></html>
        "#,
    )
}

async fn prefetch_content_two() -> Html<&'static str> {
    Html(
        r#"
        <html><body>
          <article id="content">
            <p>第二章预取测试正文。</p>
          </article>
        </body></html>
        "#,
    )
}

#[tokio::test]
async fn import_legacy_json_text_reports_progress_batches() {
    let temp = tempfile::tempdir().unwrap();
    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .unwrap();

    let sources: Vec<_> = (0..30)
        .map(|index| {
            json!({
                "bookSourceName": format!("Progress Fixture {index}"),
                "bookSourceUrl": format!("https://progress.example/{index}"),
                "enabled": true,
                "searchUrl": "/search?key={{key}}",
                "ruleSearch": {
                    "bookList": ".item",
                    "name": ".title@text",
                    "bookUrl": ".title@href"
                }
            })
        })
        .collect();
    let content = serde_json::to_string(&sources).unwrap();
    let progress_events = Arc::new(Mutex::new(Vec::new()));
    let progress_for_callback = progress_events.clone();

    let result = core
        .import_legacy_json_text_with_progress(&content, false, move |progress| {
            let progress_for_callback = progress_for_callback.clone();
            async move {
                progress_for_callback.lock().unwrap().push(progress);
            }
        })
        .await
        .unwrap();

    assert_eq!(result.imported, 30);
    let events = progress_events.lock().unwrap();
    assert!(
        events.iter().any(|event| event.processed == 25),
        "expected a progress event at the batch interval"
    );
    let final_event = events.last().expect("expected final progress event");
    assert_eq!(final_event.processed, 30);
    assert_eq!(final_event.total, 30);
    assert_eq!(final_event.imported, 30);
    assert!(final_event.done);
}

#[tokio::test]
async fn route_b_facade_imports_reads_and_persists_main_path() {
    let app = Router::new()
        .route("/search", get(search))
        .route("/book/1", get(book))
        .route("/book/1/toc", get(toc))
        .route("/chapter/1", get(content));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", addr);

    let temp = tempfile::tempdir().unwrap();
    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .unwrap();

    let import = core
        .import_legacy_json_text(
            &json!({
                "bookSourceName": "Route B Fixture",
                "bookSourceUrl": base,
                "enabled": true,
                "searchUrl": "/search?key={{key}}",
                "ruleSearch": {
                    "bookList": ".item",
                    "name": ".title@text",
                    "author": ".author@text",
                    "intro": ".intro@text",
                    "bookUrl": ".title@href"
                },
                "ruleBookInfo": {
                    "name": "h1@text",
                    "author": ".author@text",
                    "intro": "#intro@text",
                    "tocUrl": "#toc@href"
                },
                "ruleToc": {
                    "chapterList": "#chapters a",
                    "chapterName": "text",
                    "chapterUrl": "href"
                },
                "ruleContent": {
                    "content": "#content@text"
                }
            })
            .to_string(),
            false,
        )
        .await
        .unwrap();
    assert_eq!(import.imported, 1);

    let file_name = import.files[0].clone();
    let sources = core.list_sources().await.unwrap();
    assert!(sources.iter().any(|source| source.file_name == file_name));
    assert_eq!(
        core.eval_source_capabilities(&file_name, None)
            .await
            .unwrap(),
        "search,bookInfo,toc,chapterList,content,chapterContent"
    );

    let books = core.search(&file_name, "Route", 1, None).await.unwrap();
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].name, "Route B Book");

    let detail = core
        .book_info(&file_name, &books[0].book_url, None)
        .await
        .unwrap();
    assert_eq!(detail.name, "Route B Book");
    let toc_url = detail.toc_url.clone().unwrap();

    let chapters = core.chapter_list(&file_name, &toc_url, None).await.unwrap();
    assert_eq!(chapters.len(), 1);
    assert_eq!(chapters[0].name, "第一章 原生路线");

    let text = core
        .chapter_content(&file_name, &chapters[0].url, None)
        .await
        .unwrap();
    assert!(text.contains("Route B"));

    let shelf = core
        .shelf_add(
            AddBookPayload {
                name: detail.name,
                author: Some(detail.author),
                cover_url: detail.cover_url,
                intro: detail.intro,
                kind: detail.kind,
                group_id: None,
                book_url: books[0].book_url.clone(),
                source_dir: None,
                last_chapter: chapters.last().map(|chapter| chapter.name.clone()),
                source_type: Some("novel".to_string()),
            },
            &file_name,
            "Route B Fixture",
        )
        .await
        .unwrap();
    core.shelf_save_chapters(
        &shelf.id,
        chapters
            .iter()
            .enumerate()
            .map(|(index, chapter)| CachedChapter {
                index: index as i32,
                name: chapter.name.clone(),
                url: chapter.url.clone(),
                group: chapter.group.clone(),
                vip: chapter.vip,
                price: None,
                currency: None,
            })
            .collect(),
    )
    .await
    .unwrap();
    core.shelf_save_content(&shelf.id, 0, &text).await.unwrap();
    core.shelf_update_progress(
        &shelf.id,
        0,
        &chapters[0].url,
        Some(2),
        Some(0.5),
        None,
        None,
    )
    .await
    .unwrap();

    let restored = core.shelf_get(&shelf.id).await.unwrap();
    assert_eq!(restored.read_chapter_index, 0);
    assert_eq!(restored.read_page_index, 2);
    assert_eq!(
        core.shelf_get_content(&shelf.id, 0).await.unwrap(),
        Some(text)
    );
    assert_eq!(core.shelf_cached_indices(&shelf.id).await.unwrap(), vec![0]);

    server.abort();
}

#[tokio::test]
async fn legado_browser_action_captures_legacy_paragraph_comment_url() {
    let temp = tempfile::tempdir().unwrap();
    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .unwrap();

    let import = core
        .import_legacy_json_text(
            &json!({
                "bookSourceName": "Legacy Comment Fixture",
                "bookSourceUrl": "https://example.invalid",
                "enabled": true,
                "jsLib": r#"
function showCmt(bid, cid, para, date) {
  java.startBrowser(
    'https://example.invalid/comments?book_id=' + bid + '&chapter_id=' + cid + '&paragraph_id=' + para + '&date=' + date,
    'fixture paragraph'
  );
}
"#,
                "ruleContent": {
                    "content": "#content@text"
                }
            })
            .to_string(),
            false,
        )
        .await
        .unwrap();
    assert_eq!(import.imported, 1);

    let value = core
        .source_call_fn(
            &import.files[0],
            "__legado_browser_action",
            vec![json!({
                "expression": "showCmt('book-1', 'chapter-2', 'para-3', 'date-4')",
                "chapterUrl": "https://example.invalid/chapter/2",
                "chapterTitle": "Fixture Chapter",
                "chapterIndex": 1
            })],
            None,
        )
        .await
        .unwrap();

    assert_eq!(value["ok"], true);
    assert_eq!(
        value["browser"]["url"],
        "https://example.invalid/comments?book_id=book-1&chapter_id=chapter-2&paragraph_id=para-3&date=date-4"
    );
    assert_eq!(value["browser"]["title"], "fixture paragraph");
}

#[tokio::test]
async fn prefetch_chapters_respects_range_and_emits_progress() {
    let app = Router::new()
        .route("/book/1", get(book))
        .route("/book/1/toc", get(prefetch_toc))
        .route("/prefetch/chapter/1", get(prefetch_content_one))
        .route("/prefetch/chapter/2", get(prefetch_content_two));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", addr);

    let temp = tempfile::tempdir().unwrap();
    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .unwrap();

    let import = core
        .import_legacy_json_text(
            &json!({
                "bookSourceName": "Prefetch Fixture",
                "bookSourceUrl": base,
                "enabled": true,
                "searchUrl": "/search?key={{key}}",
                "ruleBookInfo": {
                    "name": "h1@text",
                    "author": ".author@text",
                    "intro": "#intro@text",
                    "tocUrl": "#toc@href"
                },
                "ruleToc": {
                    "chapterList": "#chapters a",
                    "chapterName": "text",
                    "chapterUrl": "href"
                },
                "ruleContent": {
                    "content": "#content@text"
                }
            })
            .to_string(),
            false,
        )
        .await
        .unwrap();

    let file_name = import.files[0].clone();
    let detail = core
        .book_info(&file_name, &format!("{base}/book/1"), None)
        .await
        .unwrap();
    let chapters = core
        .chapter_list(&file_name, detail.toc_url.as_deref().unwrap(), None)
        .await
        .unwrap();
    assert_eq!(chapters.len(), 2);

    let shelf = core
        .shelf_add(
            AddBookPayload {
                name: detail.name,
                author: Some(detail.author),
                cover_url: detail.cover_url,
                intro: detail.intro,
                kind: detail.kind,
                group_id: None,
                book_url: format!("{base}/book/1"),
                source_dir: None,
                last_chapter: chapters.last().map(|chapter| chapter.name.clone()),
                source_type: Some("novel".to_string()),
            },
            &file_name,
            "Prefetch Fixture",
        )
        .await
        .unwrap();

    core.shelf_save_chapters(
        &shelf.id,
        chapters
            .iter()
            .enumerate()
            .map(|(index, chapter)| CachedChapter {
                index: index as i32,
                name: chapter.name.clone(),
                url: chapter.url.clone(),
                group: chapter.group.clone(),
                vip: chapter.vip,
                price: None,
                currency: None,
            })
            .collect(),
    )
    .await
    .unwrap();

    let progress = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let progress_for_callback = progress.clone();
    let fetched = core
        .prefetch_chapters(
            &shelf.id,
            &file_name,
            None,
            Some(1),
            Some(1),
            None,
            Some(move |done, total, chapter_index| {
                progress_for_callback
                    .lock()
                    .unwrap()
                    .push((done, total, chapter_index));
            }),
        )
        .await
        .unwrap();

    assert_eq!(fetched, 1);
    assert_eq!(core.shelf_get_content(&shelf.id, 0).await.unwrap(), None);
    let cached = core.shelf_get_content(&shelf.id, 1).await.unwrap().unwrap();
    assert!(cached.contains("第二章预取测试正文"));
    assert_eq!(*progress.lock().unwrap(), vec![(1, 1, 1)]);

    server.abort();
}

#[tokio::test]
async fn stream_sources_emits_incremental_batches_with_capabilities() {
    let temp = tempfile::tempdir().unwrap();
    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .unwrap();

    for index in 0..3 {
        let source = json!({
            "bookSourceName": format!("Batch Fixture {index}"),
            "bookSourceUrl": format!("https://batch.example/{index}"),
            "searchUrl": "/search?key={{key}}",
            "ruleSearch": {
                "bookList": "$[*]",
                "name": "name",
                "author": "author",
                "bookUrl": "bookUrl"
            }
        });
        core.import_legacy_json_text(&source.to_string(), false)
            .await
            .unwrap();
    }

    let batches = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let batches_for_callback = batches.clone();
    let total = core
        .stream_sources(2, true, move |items, done, total| {
            let batches = batches_for_callback.clone();
            async move {
                batches.lock().unwrap().push((
                    items.len(),
                    done,
                    total,
                    items
                        .iter()
                        .all(|item| item.capabilities.iter().any(|cap| cap == "search")),
                ));
            }
        })
        .await
        .unwrap();

    let batches = batches.lock().unwrap();
    assert_eq!(total, 3);
    assert!(
        batches.iter().any(|(_, done, _, _)| !done),
        "expected at least one non-final batch: {batches:?}"
    );
    assert_eq!(batches.last().map(|(_, done, _, _)| *done), Some(true));
    assert_eq!(batches.last().and_then(|(_, _, total, _)| *total), Some(3));
    assert!(
        batches.iter().all(|(_, _, _, has_search)| *has_search),
        "capabilities should be carried in every streamed meta batch: {batches:?}"
    );
}

#[tokio::test]
#[ignore = "live network test for the user-provided Legado source"]
async fn live_yckceo_3417_novel_reading_path() {
    let temp = tempfile::tempdir().unwrap();
    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .unwrap();

    let import = core
        .import_legacy_json_url(
            "https://www.yckceo.com/yuedu/shuyuan/json/id/3417.json",
            false,
        )
        .await
        .unwrap();
    assert_eq!(import.imported, 1);

    let file_name = import.files[0].clone();
    let books = core.search(&file_name, "剑来", 1, None).await.unwrap();
    assert!(!books.is_empty(), "search should return at least one book");
    assert!(
        books.iter().any(|book| book.name.contains("剑来")),
        "search results should include 剑来: {books:?}"
    );

    let book = books
        .iter()
        .find(|book| book.name == "剑来")
        .unwrap_or(&books[0]);
    let detail = core
        .book_info(&file_name, &book.book_url, None)
        .await
        .unwrap();
    assert!(!detail.name.trim().is_empty());

    let toc_url = detail.toc_url.as_deref().unwrap_or(&book.book_url);
    let chapters = core.chapter_list(&file_name, toc_url, None).await.unwrap();
    assert!(
        !chapters.is_empty(),
        "chapter list should not be empty for {toc_url}"
    );

    let content = core
        .chapter_content(&file_name, &chapters[0].url, None)
        .await
        .unwrap();
    assert!(
        content.chars().count() > 100,
        "chapter content should contain readable text, got: {content:?}"
    );
}

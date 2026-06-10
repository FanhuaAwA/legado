use axum::{response::Html, routing::get, Router};
use reader_core::{AddBookPayload, CachedChapter, ReaderCore, ReaderCoreOptions};
use serde_json::json;

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

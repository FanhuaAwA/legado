use axum::{response::Html, routing::get, Router};
use reader_core::crawler::fetcher::HttpMethod;
use reader_core::crawler::url_analyzer::analyze_url;
use reader_core::model::book_source::{book_source_from_value, BookSource};
use reader_core::model::rule::{SearchRule, TocRule};
use reader_core::parser::rule_engine::RuleEngine;
use reader_core::parser::{html, jsonpath};
use serde_json::json;

#[test]
fn book_source_deserializes_stringified_rule_objects() {
    let source: BookSource = serde_json::from_value(json!({
        "bookSourceName": "String rules",
        "bookSourceUrl": "https://example.test",
        "ruleSearch": "{\"bookList\":\".item\",\"name\":\".title@text\"}"
    }))
    .unwrap();

    let rule = source.rule_search.unwrap();
    assert_eq!(rule.book_list.as_deref(), Some(".item"));
    assert_eq!(rule.name.as_deref(), Some(".title@text"));
}

#[test]
fn legacy_book_source_fields_are_migrated_to_current_shape() {
    let source = book_source_from_value(json!({
        "bookSourceName": "Legacy",
        "bookSourceUrl": "https://legacy.example",
        "httpUserAgent": "LegacyUA",
        "ruleSearchUrl": "/search?keyword=searchKey&page=searchPage@Header:{\"X-Legacy\":\"1\"}",
        "ruleSearchList": ".item",
        "ruleSearchName": ".name@text",
        "ruleSearchAuthor": ".author@text",
        "ruleBookName": "h1@text",
        "ruleChapterList": "a",
        "ruleChapterName": "a@text",
        "ruleContentUrl": "a@href",
        "ruleBookContent": "#content@text"
    }))
    .unwrap();

    assert_eq!(
        source.search_url.as_deref(),
        Some("/search?keyword={{key}}&page={{page}},{\"headers\":{\"X-Legacy\":\"1\"}}")
    );
    assert_eq!(
        source
            .rule_search
            .as_ref()
            .and_then(|rule| rule.book_list.as_deref()),
        Some(".item")
    );
    assert_eq!(
        source
            .rule_book_info
            .as_ref()
            .and_then(|rule| rule.name.as_deref()),
        Some("h1@text")
    );
    assert_eq!(
        source
            .rule_toc
            .as_ref()
            .and_then(|rule| rule.chapter_url.as_deref()),
        Some("a@href")
    );
}

#[test]
fn url_analyzer_supports_inline_js_page_choices_headers_and_response_type() {
    let source = BookSource {
        book_source_name: "URL compat".to_string(),
        book_source_url: "https://a.test/root/".to_string(),
        header: Some("@js:JSON.stringify({'X-Token':'ok'})".to_string()),
        ..Default::default()
    };

    let spec = analyze_url(
        "/search?q={{key}}&page=<1,2,3>,{\"headers\":{\"Referer\":\"https://a.test\"},\"retry\":3,\"type\":\"hex\"}",
        "斗破",
        2,
        &source.book_source_url,
        &source,
    )
    .unwrap();

    assert_eq!(spec.method, HttpMethod::GET);
    assert_eq!(
        spec.url,
        "https://a.test/search?q=%E6%96%97%E7%A0%B4&page=2"
    );
    assert_eq!(spec.retry, 3);
    assert_eq!(spec.response_type.as_deref(), Some("hex"));
    assert!(spec
        .headers
        .iter()
        .any(|(name, value)| name == "X-Token" && value == "ok"));
}

#[test]
fn html_rule_split_ignores_delimiters_inside_attribute_selectors() {
    let doc = html::parse_document(r#"<div data-x="a&&b">Bad</div><span>Good</span>"#);

    assert_eq!(
        html::select_text_list(&doc, r#"div[data-x="a&&b"]@text||span@text"#),
        vec!["Bad".to_string()]
    );
}

#[test]
fn jsonpath_supports_embedded_path_templates() {
    let value = json!({"data":{"name":"书名","author":"作者"}});

    assert_eq!(
        jsonpath::jsonpath_first_string(&value, "作者：{$.data.author}"),
        Some("作者：作者".to_string())
    );
}

#[test]
fn chapter_list_strips_css_mode_prefix() {
    let engine = RuleEngine::new().unwrap();
    let source = BookSource {
        book_source_name: "TOC".to_string(),
        book_source_url: "https://toc.example".to_string(),
        rule_toc: Some(TocRule {
            chapter_list: Some("@css:.dirList li a".to_string()),
            chapter_name: Some("text".to_string()),
            chapter_url: Some("href".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    };

    let (chapters, _) = engine.chapter_list(
        &source,
        r#"<ul class="dirList"><li><a href="/c1.html">第一章</a></li></ul>"#,
        "https://toc.example/book/",
    );

    assert_eq!(chapters.len(), 1);
    assert_eq!(chapters[0].title, "第一章");
    assert_eq!(chapters[0].url, "https://toc.example/c1.html");
}

async fn search_ok() -> Html<&'static str> {
    Html(
        r#"<div class="item"><a class="title" href="/book/1">搜索书</a><span class="author">作者</span></div>"#,
    )
}

async fn empty_page() -> Html<&'static str> {
    Html("")
}

#[tokio::test]
async fn source_availability_is_valid_when_search_has_results() {
    let app = Router::new()
        .route("/search-ok", get(search_ok))
        .route("/empty", get(empty_page));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let temp = tempfile::tempdir().unwrap();
    let service = reader_core::service::book_service::BookService::new(
        reader_core::crawler::http_client::HttpClient::new(5, None).unwrap(),
        RuleEngine::new().unwrap(),
        reader_core::storage::cache::file_cache::FileCache::new(temp.path().join("cache")),
        temp.path().to_str().unwrap(),
    );

    let source = BookSource {
        book_source_name: "Source".to_string(),
        book_source_url: format!("http://{}", addr),
        search_url: Some("/search-ok".to_string()),
        rule_search: Some(SearchRule {
            check_key_word: Some("书".to_string()),
            book_list: Some(".item".to_string()),
            name: Some(".title@text".to_string()),
            author: Some(".author@text".to_string()),
            book_url: Some(".title@href".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    };

    let result = service
        .test_book_source_availability("default", &source, Some("书"))
        .await;

    server.abort();

    assert!(result.valid);
    assert!(result.search_ok);
}

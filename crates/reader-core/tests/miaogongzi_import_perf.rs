use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use futures::{stream, StreamExt};
use reader_core::{ReaderCore, ReaderCoreOptions};
use serde_json::Value;
use tokio::fs;

const MIAOGONGZI_SUBSCRIPTION_URL: &str = "http://yuedu.miaogongzi.net/shuyuan/miaogongziDY.json";
const FETCH_CONCURRENCY: usize = 8;

#[derive(Debug)]
struct ResolvedPackage {
    url: String,
    content: String,
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(45))
        .user_agent(
            "Mozilla/5.0 (Linux; Android 12; Mobile) AppleWebKit/537.36 Chrome/120 Mobile Safari/537.36",
        )
        .build()
        .expect("reqwest client should build")
}

async fn fetch_text(client: &reqwest::Client, url: &str) -> Result<String> {
    let response = client
        .get(url)
        .header("Accept", "text/html,application/json,text/plain,*/*")
        .send()
        .await
        .with_context(|| format!("failed to fetch {url}"))?
        .error_for_status()
        .with_context(|| format!("fetch returned non-success status for {url}"))?;
    response
        .text()
        .await
        .with_context(|| format!("failed to read response body for {url}"))
}

fn normalize_http_url(value: &str) -> Option<String> {
    let parsed = url::Url::parse(value.trim()).ok()?;
    match parsed.scheme() {
        "http" | "https" => Some(parsed.to_string()),
        _ => None,
    }
}

fn read_import_page_url(value: &str) -> Option<String> {
    let normalized = value.replace("&&", "\n").replace(',', "\n");
    for segment in normalized
        .lines()
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let candidate = segment.rsplit("::").next().unwrap_or(segment);
        if let Some(url) = normalize_http_url(candidate) {
            return Some(url);
        }
    }
    None
}

fn read_subscription_page_urls(value: &Value) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut urls = Vec::new();
    let items = value
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_else(|| std::slice::from_ref(value));
    for item in items {
        if let Some(source_url) = item
            .get("sourceUrl")
            .and_then(Value::as_str)
            .and_then(normalize_http_url)
        {
            if seen.insert(source_url.clone()) {
                urls.push(source_url);
            }
        }
        if let Some(sort_url) = item
            .get("sortUrl")
            .and_then(Value::as_str)
            .and_then(read_import_page_url)
        {
            if seen.insert(sort_url.clone()) {
                urls.push(sort_url);
            }
        }
    }
    urls
}

fn read_yuedu_booksource_url(value: &str) -> Option<String> {
    let parsed = url::Url::parse(&value.replace("&amp;", "&")).ok()?;
    if parsed.scheme() != "yuedu"
        || parsed.host_str() != Some("booksource")
        || parsed.path() != "/importonline"
    {
        return None;
    }
    parsed
        .query_pairs()
        .find_map(|(key, value)| (key == "src").then(|| value.into_owned()))
        .and_then(|url| normalize_http_url(&url))
}

fn extract_booksource_urls_from_html(html: &str) -> Vec<String> {
    let pattern = regex::Regex::new(r#"yuedu://booksource/importonline\?src=[^"'\s<>]+"#)
        .expect("regex should compile");
    let mut seen = HashSet::new();
    let mut urls = Vec::new();
    for item in pattern.find_iter(html) {
        if let Some(url) = read_yuedu_booksource_url(item.as_str()) {
            if seen.insert(url.clone()) {
                urls.push(url);
            }
        }
    }
    urls
}

async fn resolve_miaogongzi_packages() -> Result<Vec<ResolvedPackage>> {
    let client = http_client();
    let subscription_text = fetch_text(&client, MIAOGONGZI_SUBSCRIPTION_URL).await?;
    let subscription: Value =
        serde_json::from_str(&subscription_text).context("subscription JSON should parse")?;
    let page_urls = read_subscription_page_urls(&subscription);
    anyhow::ensure!(
        !page_urls.is_empty(),
        "subscription should expose import pages"
    );

    let pages = stream::iter(page_urls.into_iter().map(|url| {
        let client = client.clone();
        async move {
            let text = fetch_text(&client, &url).await?;
            Ok::<_, anyhow::Error>(extract_booksource_urls_from_html(&text))
        }
    }))
    .buffer_unordered(FETCH_CONCURRENCY)
    .collect::<Vec<_>>()
    .await;

    let mut seen = HashSet::new();
    let mut package_urls = Vec::new();
    for page in pages {
        for url in page? {
            if seen.insert(url.clone()) {
                package_urls.push(url);
            }
        }
    }
    anyhow::ensure!(
        !package_urls.is_empty(),
        "import pages should expose booksource packages"
    );

    let packages = stream::iter(package_urls.into_iter().map(|url| {
        let client = client.clone();
        async move {
            let content = fetch_text(&client, &url).await?;
            serde_json::from_str::<Value>(&content)
                .with_context(|| format!("booksource package JSON should parse: {url}"))?;
            Ok::<_, anyhow::Error>(ResolvedPackage { url, content })
        }
    }))
    .buffer_unordered(FETCH_CONCURRENCY)
    .collect::<Vec<_>>()
    .await;

    packages.into_iter().collect()
}

fn combine_legacy_json_contents<'a>(contents: impl IntoIterator<Item = &'a str>) -> Result<String> {
    let mut values = Vec::new();
    for content in contents {
        let parsed: Value =
            serde_json::from_str(content).context("booksource package JSON should parse")?;
        match parsed {
            Value::Array(items) => values.extend(items),
            other => values.push(other),
        }
    }
    serde_json::to_string(&values).context("combined package JSON should serialize")
}

fn combine_package_contents(packages: &[ResolvedPackage]) -> Result<String> {
    combine_legacy_json_contents(packages.iter().map(|package| package.content.as_str()))
}

async fn import_sequential(packages: &[ResolvedPackage]) -> Result<(u128, usize, usize)> {
    let temp = tempfile::tempdir().context("temp dir should be created")?;
    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .context("reader core should initialize")?;
    let started = Instant::now();
    let mut imported = 0;
    let mut skipped = 0;
    for package in packages {
        let result = core
            .import_legacy_json_text(&package.content, false)
            .await
            .with_context(|| format!("sequential import should succeed: {}", package.url))?;
        imported += result.imported;
        skipped += result.skipped;
    }
    Ok((started.elapsed().as_millis(), imported, skipped))
}

async fn write_package_files(
    packages: &[ResolvedPackage],
) -> Result<(tempfile::TempDir, Vec<PathBuf>)> {
    let source_dir =
        tempfile::tempdir().context("local import source temp dir should be created")?;
    let mut paths = Vec::with_capacity(packages.len());
    for (index, package) in packages.iter().enumerate() {
        let path = source_dir
            .path()
            .join(format!("miaogongzi-package-{index}.json"));
        fs::write(&path, &package.content)
            .await
            .with_context(|| format!("local source package should be written: {}", package.url))?;
        paths.push(path);
    }
    Ok((source_dir, paths))
}

async fn import_local_files_sequential(
    packages: &[ResolvedPackage],
) -> Result<(u128, usize, usize)> {
    let (_source_dir, paths) = write_package_files(packages).await?;
    let temp = tempfile::tempdir().context("temp dir should be created")?;
    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .context("reader core should initialize")?;
    let started = Instant::now();
    let mut imported = 0;
    let mut skipped = 0;
    for path in paths {
        let content = fs::read_to_string(&path)
            .await
            .with_context(|| format!("local source package should be read: {}", path.display()))?;
        let result = core
            .import_legacy_json_text(&content, false)
            .await
            .with_context(|| {
                format!("local sequential import should succeed: {}", path.display())
            })?;
        imported += result.imported;
        skipped += result.skipped;
    }
    Ok((started.elapsed().as_millis(), imported, skipped))
}

async fn import_local_files_combined(packages: &[ResolvedPackage]) -> Result<(u128, usize, usize)> {
    let (_source_dir, paths) = write_package_files(packages).await?;
    let temp = tempfile::tempdir().context("temp dir should be created")?;
    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .context("reader core should initialize")?;
    let started = Instant::now();
    let reads = stream::iter(paths.into_iter().map(|path| async move {
        let label = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("local-source.json")
            .to_string();
        let content = fs::read_to_string(&path)
            .await
            .with_context(|| format!("local source package should be read: {}", path.display()))?;
        Ok::<_, anyhow::Error>((label, content))
    }))
    .buffer_unordered(FETCH_CONCURRENCY)
    .collect::<Vec<_>>()
    .await;
    let mut items = Vec::with_capacity(reads.len());
    for read in reads {
        items.push(read?);
    }
    let result = core
        .import_legacy_json_texts(&items, false)
        .await
        .context("local combined import should succeed")?;
    Ok((
        started.elapsed().as_millis(),
        result.imported,
        result.skipped,
    ))
}

async fn import_combined(packages: &[ResolvedPackage]) -> Result<(u128, usize, usize)> {
    let temp = tempfile::tempdir().context("temp dir should be created")?;
    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .context("reader core should initialize")?;
    let items = packages
        .iter()
        .map(|package| (package.url.clone(), package.content.clone()))
        .collect::<Vec<_>>();
    let started = Instant::now();
    let result = core
        .import_legacy_json_texts(&items, false)
        .await
        .context("combined import should succeed")?;
    Ok((
        started.elapsed().as_millis(),
        result.imported,
        result.skipped,
    ))
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "live network against yuedu.miaogongzi.net; run manually for import performance checks"]
async fn miaogongzi_subscription_import_sequential_vs_combined() -> Result<()> {
    let resolve_started = Instant::now();
    let packages = resolve_miaogongzi_packages().await?;
    let resolve_ms = resolve_started.elapsed().as_millis();
    let entry_count = combine_package_contents(&packages)?
        .parse::<Value>()?
        .as_array()
        .map(Vec::len)
        .unwrap_or_default();
    let (sequential_ms, sequential_imported, sequential_skipped) =
        import_sequential(&packages).await?;
    let (combined_ms, combined_imported, combined_skipped) = import_combined(&packages).await?;
    let (local_sequential_ms, local_sequential_imported, local_sequential_skipped) =
        import_local_files_sequential(&packages).await?;
    let (local_combined_ms, local_combined_imported, local_combined_skipped) =
        import_local_files_combined(&packages).await?;
    let speedup = sequential_ms as f64 / combined_ms.max(1) as f64;
    let local_speedup = local_sequential_ms as f64 / local_combined_ms.max(1) as f64;

    eprintln!(
        "miaogongzi_import_perf packages={} entries={} resolve_ms={} sequential_ms={} sequential_imported={} sequential_skipped={} combined_ms={} combined_imported={} combined_skipped={} speedup={:.2}x local_sequential_ms={} local_sequential_imported={} local_sequential_skipped={} local_combined_ms={} local_combined_imported={} local_combined_skipped={} local_speedup={:.2}x",
        packages.len(),
        entry_count,
        resolve_ms,
        sequential_ms,
        sequential_imported,
        sequential_skipped,
        combined_ms,
        combined_imported,
        combined_skipped,
        speedup,
        local_sequential_ms,
        local_sequential_imported,
        local_sequential_skipped,
        local_combined_ms,
        local_combined_imported,
        local_combined_skipped,
        local_speedup,
    );

    assert!(!packages.is_empty(), "subscription should resolve packages");
    assert!(
        entry_count > 0,
        "resolved packages should contain source entries"
    );
    assert!(
        combined_imported > 0,
        "combined import should import sources"
    );
    assert!(
        local_combined_imported > 0,
        "local combined import should import sources"
    );
    Ok(())
}

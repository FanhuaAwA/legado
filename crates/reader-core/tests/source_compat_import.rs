use reader_core::{ReaderCore, ReaderCoreOptions, SourceRuntimeKind};

/// 验证本地书源可成功导入并正确解析字段

fn read_source_fixture(path: &str) -> String {
    std::fs::read_to_string(path).expect("fixture file must be readable")
}

#[tokio::test]
async fn shuqi_source_live_search() {
    let temp = tempfile::tempdir().unwrap();
    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .unwrap();

    let content = read_source_fixture(r"E:\Book\书旗书源\sqxs260128_0ee680c1.json");
    let result = core.import_legacy_json_text(&content, false).await.unwrap();
    assert!(result.imported > 0);
    assert!(result.files.len() > 0);

    let file_name = &result.files[0];
    let search_result = core.search(file_name, "系统", 1, None).await;

    match search_result {
        Ok(books) => {
            assert!(!books.is_empty(), "书旗搜索应返回结果");
            for book in &books {
                assert!(!book.name.is_empty(), "书名不应为空");
                assert!(!book.book_url.is_empty(), "bookUrl不应为空");
            }
        }
        Err(err) => {
            eprintln!("书旗搜索失败（可能是源站不可达或规则已失效）: {err:?}");
        }
    }
}

#[tokio::test]
#[ignore = "live network: requires 七猫源站可用"]
async fn qimao_source_live_search() {
    let temp = tempfile::tempdir().unwrap();
    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .unwrap();

    let content = read_source_fixture(r"E:\Book\七猫书源\qmxs260128_432b9f7e.json");
    let result = core.import_legacy_json_text(&content, false).await.unwrap();
    assert!(result.imported > 0);

    let file_name = &result.files[0];
    let search_result = core.search(file_name, "测试", 1, None).await;
    match search_result {
        Ok(books) => {
            eprintln!("七猫搜索返回 {} 条结果", books.len());
        }
        Err(err) => {
            eprintln!("七猫搜索失败: {err:?}");
        }
    }
}

#[tokio::test]
async fn shuqi_source_imports_and_parses_fields() {
    let temp = tempfile::tempdir().unwrap();
    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .unwrap();

    let content = read_source_fixture(r"E:\Book\书旗书源\sqxs260128_0ee680c1.json");
    let result = core.import_legacy_json_text(&content, false).await.unwrap();

    assert!(result.imported > 0, "书旗书源应能成功导入");
    assert!(result.errors.is_empty(), "书旗书源导入无错误: {:?}", result.errors);

    let sources = core.list_sources().await.unwrap();
    let shuqi = sources
        .iter()
        .find(|s| s.name.contains("书旗"))
        .expect("书旗书源应在列表中");

    assert!(shuqi.name.contains("书旗小说"));
    assert!(matches!(shuqi.runtime, SourceRuntimeKind::LegadoRule));
    assert!(shuqi.enabled, "书旗书源应默认为启用");
    assert!(!shuqi.file_name.is_empty(), "应有文件名");
}

#[tokio::test]
async fn qimao_source_imports_and_parses_fields() {
    let temp = tempfile::tempdir().unwrap();
    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .unwrap();

    let content = read_source_fixture(r"E:\Book\七猫书源\qmxs260128_432b9f7e.json");
    let result = core.import_legacy_json_text(&content, false).await.unwrap();

    assert!(result.imported > 0, "七猫书源应能成功导入");
    assert!(result.errors.is_empty(), "七猫书源导入无错误: {:?}", result.errors);

    let sources = core.list_sources().await.unwrap();
    let qimao = sources
        .iter()
        .find(|s| s.name.contains("七猫"))
        .expect("七猫书源应在列表中");

    assert!(qimao.name.contains("七猫小说"));
    assert!(matches!(qimao.runtime, SourceRuntimeKind::LegadoRule));
    assert!(qimao.enabled);
}

#[tokio::test]
async fn fanqie_source_imports_and_parses_fields() {
    let temp = tempfile::tempdir().unwrap();
    let core = ReaderCore::new(ReaderCoreOptions::new(temp.path()))
        .await
        .unwrap();

    let content = read_source_fixture(r"E:\Book\番茄书源\fqfix0529_45469384.json");
    let result = core.import_legacy_json_text(&content, false).await.unwrap();

    assert!(result.imported > 0, "番茄书源应能成功导入");

    let sources = core.list_sources().await.unwrap();
    let fanqie = sources
        .iter()
        .find(|s| s.name.contains("番茄"))
        .expect("番茄书源应在列表中");

    assert!(fanqie.name.contains("番茄"));
    assert!(matches!(fanqie.runtime, SourceRuntimeKind::LegadoRule));
    assert!(fanqie.enabled);
}

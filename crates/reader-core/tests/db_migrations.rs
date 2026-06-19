use reader_core::storage::db::init_pool;

const LEGACY_BOOK_SOURCE_LIST_INDEX_CHECKSUM_HEX: &str =
    "ad44f866f5ba7d963d5675362a49a82a771c2ae67746f3279c2af2bb0d88d25065cde654183903faf3158b9f87d461fa";

#[tokio::test]
async fn init_pool_repairs_legacy_book_source_index_checksum() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("reader.db");
    let database_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = init_pool(&database_url).await.unwrap();
    let legacy_checksum = hex::decode(LEGACY_BOOK_SOURCE_LIST_INDEX_CHECKSUM_HEX).unwrap();
    sqlx::query("UPDATE _sqlx_migrations SET checksum = ? WHERE version = 4")
        .bind(&legacy_checksum)
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;

    let pool = init_pool(&database_url).await.unwrap();
    let checksum: Vec<u8> =
        sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version = 4")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_ne!(checksum, legacy_checksum);

    let index_name: Option<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'index' AND name = 'idx_book_sources_user_updated_url'",
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert_eq!(
        index_name.as_deref(),
        Some("idx_book_sources_user_updated_url")
    );
}

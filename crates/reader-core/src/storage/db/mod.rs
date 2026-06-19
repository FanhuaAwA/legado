pub mod repo;

use sqlx::{
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::str::FromStr;

static MIGRATOR: Migrator = sqlx::migrate!("src/storage/db/migrations");
const BOOK_SOURCE_LIST_INDEX_MIGRATION_VERSION: i64 = 4;
const BOOK_SOURCE_LIST_INDEX_MIGRATION_DESCRIPTION: &str = "book source list index";
const LEGACY_BOOK_SOURCE_LIST_INDEX_CHECKSUM_HEX: &str =
    "ad44f866f5ba7d963d5675362a49a82a771c2ae67746f3279c2af2bb0d88d25065cde654183903faf3158b9f87d461fa";

pub async fn init_pool(database_url: &str) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    repair_legacy_book_source_index_migration(&pool).await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

async fn repair_legacy_book_source_index_migration(pool: &SqlitePool) -> anyhow::Result<()> {
    let has_migrations_table: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_optional(pool)
    .await?;
    if has_migrations_table.is_none() {
        return Ok(());
    }

    let Some(current) = MIGRATOR
        .iter()
        .find(|migration| migration.version == BOOK_SOURCE_LIST_INDEX_MIGRATION_VERSION)
    else {
        return Ok(());
    };
    let Some((stored_checksum, success)) = sqlx::query_as::<_, (Vec<u8>, bool)>(
        "SELECT checksum, success FROM _sqlx_migrations WHERE version = ? AND description = ?",
    )
    .bind(BOOK_SOURCE_LIST_INDEX_MIGRATION_VERSION)
    .bind(BOOK_SOURCE_LIST_INDEX_MIGRATION_DESCRIPTION)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(());
    };
    if !success || stored_checksum == current.checksum.as_ref() {
        return Ok(());
    }

    let legacy_checksum = hex::decode(LEGACY_BOOK_SOURCE_LIST_INDEX_CHECKSUM_HEX)?;
    if stored_checksum != legacy_checksum {
        return Ok(());
    }

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_book_sources_user_updated_url
        ON book_sources(user_ns, updated_at DESC, book_source_url DESC)
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query("UPDATE _sqlx_migrations SET checksum = ? WHERE version = ? AND description = ?")
        .bind(current.checksum.as_ref())
        .bind(BOOK_SOURCE_LIST_INDEX_MIGRATION_VERSION)
        .bind(BOOK_SOURCE_LIST_INDEX_MIGRATION_DESCRIPTION)
        .execute(pool)
        .await?;
    Ok(())
}

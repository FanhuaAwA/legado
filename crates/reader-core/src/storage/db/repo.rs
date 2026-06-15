use crate::error::error::AppError;
use crate::model::book_source::BookSource;
use crate::util::time::now_ts;
use sqlx::{Row, SqlitePool};

const UPSERT_BOOK_SOURCE_SQL: &str =
    "INSERT INTO book_sources (user_ns, book_source_url, book_source_name, json, updated_at) VALUES (?1, ?2, ?3, ?4, ?5) \
     ON CONFLICT(user_ns, book_source_url) DO UPDATE SET book_source_name=excluded.book_source_name, json=excluded.json, updated_at=excluded.updated_at";

#[derive(Clone)]
pub struct BookSourceRepo {
    pool: SqlitePool,
}

#[derive(Debug, Clone)]
pub struct BookSourceListRow {
    pub book_source_url: String,
    pub book_source_name: String,
    pub json: String,
    pub updated_at: i64,
}

impl BookSourceRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(
        &self,
        user_ns: &str,
        source: &BookSource,
        json: &str,
    ) -> Result<(), AppError> {
        sqlx::query(UPSERT_BOOK_SOURCE_SQL)
            .bind(user_ns)
            .bind(&source.book_source_url)
            .bind(&source.book_source_name)
            .bind(json)
            .bind(now_ts())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn upsert_many(
        &self,
        user_ns: &str,
        sources: &[(BookSource, String)],
    ) -> Result<(), AppError> {
        if sources.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;
        let updated_at = now_ts();
        for (source, json) in sources {
            sqlx::query(UPSERT_BOOK_SOURCE_SQL)
                .bind(user_ns)
                .bind(&source.book_source_url)
                .bind(&source.book_source_name)
                .bind(json)
                .bind(updated_at)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn delete(&self, user_ns: &str, book_source_url: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM book_sources WHERE user_ns=?1 AND book_source_url=?2")
            .bind(user_ns)
            .bind(book_source_url)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_all(&self, user_ns: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM book_sources WHERE user_ns=?1")
            .bind(user_ns)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get(
        &self,
        user_ns: &str,
        book_source_url: &str,
    ) -> Result<Option<String>, AppError> {
        let row =
            sqlx::query("SELECT json FROM book_sources WHERE user_ns=?1 AND book_source_url=?2")
                .bind(user_ns)
                .bind(book_source_url)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| r.get::<String, _>("json")))
    }

    pub async fn list(&self, user_ns: &str) -> Result<Vec<String>, AppError> {
        let rows =
            sqlx::query("SELECT json FROM book_sources WHERE user_ns=?1 ORDER BY updated_at DESC")
                .bind(user_ns)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows
            .into_iter()
            .map(|r| r.get::<String, _>("json"))
            .collect())
    }

    pub async fn list_page_after(
        &self,
        user_ns: &str,
        limit: usize,
        cursor: Option<(i64, &str)>,
    ) -> Result<Vec<BookSourceListRow>, AppError> {
        let cursor_updated_at = cursor.map(|(updated_at, _)| updated_at);
        let cursor_url = cursor.map(|(_, url)| url);
        let rows = sqlx::query(
            "SELECT book_source_url, book_source_name, json, updated_at \
             FROM book_sources \
             WHERE user_ns=?1 \
               AND (?2 IS NULL OR updated_at < ?2 OR (updated_at = ?2 AND book_source_url < ?3)) \
             ORDER BY updated_at DESC, book_source_url DESC \
             LIMIT ?4",
        )
        .bind(user_ns)
        .bind(cursor_updated_at)
        .bind(cursor_url)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| BookSourceListRow {
                book_source_url: row.get("book_source_url"),
                book_source_name: row.get("book_source_name"),
                json: row.get("json"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }

    pub async fn copy_to(&self, from_ns: &str, to_ns: &str) -> Result<i64, AppError> {
        let rows = sqlx::query("SELECT book_source_url, book_source_name, json, updated_at FROM book_sources WHERE user_ns=?1")
            .bind(from_ns)
            .fetch_all(&self.pool)
            .await?;
        let count = rows.len() as i64;
        let mut tx = self.pool.begin().await?;
        for row in rows {
            let url: String = row.get("book_source_url");
            let name: String = row.get("book_source_name");
            let json: String = row.get("json");
            let updated_at: i64 = row.get("updated_at");
            sqlx::query(UPSERT_BOOK_SOURCE_SQL)
                .bind(to_ns)
                .bind(&url)
                .bind(&name)
                .bind(&json)
                .bind(updated_at)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(count)
    }
}

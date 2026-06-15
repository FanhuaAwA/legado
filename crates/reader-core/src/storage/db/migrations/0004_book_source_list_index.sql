CREATE INDEX IF NOT EXISTS idx_book_sources_user_updated_url
ON book_sources(user_ns, updated_at DESC, book_source_url DESC);

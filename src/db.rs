use anyhow::{Context, Result};
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use sentinel_ioc::{ExtractedIndicator, IndicatorType};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;

use crate::types::*;

pub struct Db {
    conn: Connection,
}

/// Parse SQLite timestamp string (format: "YYYY-MM-DD HH:MM:SS") to UTC DateTime.
fn parse_ts(s: &str) -> Option<DateTime<Utc>> {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|dt| DateTime::from_naive_utc_and_offset(dt, Utc))
}

fn parse_db_datetime(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|| parse_ts(s))
}

fn indicator_type_to_db(indicator_type: IndicatorType) -> &'static str {
    match indicator_type {
        IndicatorType::Ipv4 => "Ipv4",
        IndicatorType::Ipv6 => "Ipv6",
        IndicatorType::Domain => "Domain",
        IndicatorType::Url => "Url",
        IndicatorType::Email => "Email",
        IndicatorType::Md5 => "Md5",
        IndicatorType::Sha1 => "Sha1",
        IndicatorType::Sha256 => "Sha256",
        IndicatorType::Cve => "Cve",
        IndicatorType::MitreAttackTechnique => "MitreAttackTechnique",
        IndicatorType::OnionDomain => "OnionDomain",
        IndicatorType::OnionUrl => "OnionUrl",
        IndicatorType::CryptoWallet => "CryptoWallet",
        IndicatorType::CloudAccessKey => "CloudAccessKey",
        IndicatorType::Unknown => "Unknown",
    }
}

fn indicator_type_from_db(value: &str) -> IndicatorType {
    match value {
        "Ipv4" => IndicatorType::Ipv4,
        "Ipv6" => IndicatorType::Ipv6,
        "Domain" => IndicatorType::Domain,
        "Url" => IndicatorType::Url,
        "Email" => IndicatorType::Email,
        "Md5" => IndicatorType::Md5,
        "Sha1" => IndicatorType::Sha1,
        "Sha256" => IndicatorType::Sha256,
        "Cve" => IndicatorType::Cve,
        "MitreAttackTechnique" => IndicatorType::MitreAttackTechnique,
        "OnionDomain" => IndicatorType::OnionDomain,
        "OnionUrl" => IndicatorType::OnionUrl,
        "CryptoWallet" => IndicatorType::CryptoWallet,
        "CloudAccessKey" => IndicatorType::CloudAccessKey,
        _ => IndicatorType::Unknown,
    }
}

fn indicator_types_to_json(types: &[IndicatorType]) -> Result<String> {
    let values = types
        .iter()
        .map(|indicator_type| indicator_type_to_db(*indicator_type))
        .collect::<Vec<_>>();
    serde_json::to_string(&values).map_err(Into::into)
}

fn indicator_types_from_json(value: &str) -> Vec<IndicatorType> {
    serde_json::from_str::<Vec<String>>(value)
        .unwrap_or_default()
        .into_iter()
        .map(|value| indicator_type_from_db(&value))
        .collect()
}

fn risk_score_from_enrichment(result: &sentinel_enrichment::EnrichmentResult) -> Option<i64> {
    if let Some(score) = result.score {
        return Some(i64::from(score).clamp(0, 100));
    }

    let verdict = result.verdict.as_deref().unwrap_or("").to_ascii_lowercase();
    if verdict.contains("known exploited") {
        return Some(90);
    }

    match result.reputation {
        sentinel_enrichment::Reputation::KnownRansomware
        | sentinel_enrichment::Reputation::KnownC2
        | sentinel_enrichment::Reputation::KnownMalware
        | sentinel_enrichment::Reputation::KnownPhishing
        | sentinel_enrichment::Reputation::Malicious => Some(85),
        sentinel_enrichment::Reputation::Suspicious => Some(60),
        sentinel_enrichment::Reputation::KnownScanner => Some(25),
        sentinel_enrichment::Reputation::Benign => Some(10),
        sentinel_enrichment::Reputation::Unknown => None,
    }
}

fn feed_item_hash(feed: &Feed, item: &FetchedFeedItem) -> String {
    let hash_input = format!(
        "{}:{}:{}:{}:{}:{}",
        feed.id,
        item.url.as_deref().unwrap_or(""),
        item.title.as_deref().unwrap_or(""),
        item.date.map(|dt| dt.to_rfc3339()).unwrap_or_default(),
        item.description.as_deref().unwrap_or(""),
        item.raw_json.as_deref().unwrap_or("")
    );
    let mut hasher = Sha256::new();
    hasher.update(hash_input.as_bytes());
    hex::encode(hasher.finalize())
}

impl Db {
    #[cfg(test)]
    pub fn new_in_memory_for_tests() -> Self {
        Self {
            conn: Connection::open_in_memory().unwrap(),
        }
    }

    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("opening database at {}", path.display()))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        Ok(Self { conn })
    }

    pub fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(include_str!("schema.sql"))?;
        let first_run = self.is_first_run_database()?;
        self.apply_catalog_updates()?;
        self.ensure_builtin_enrichment_providers()?;
        if first_run {
            self.conn.execute_batch(include_str!("seed.sql"))?;
        }
        Ok(())
    }

    fn apply_catalog_updates(&self) -> Result<()> {
        let catalog_marker: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM app_meta WHERE key = 'catalog_seed_version'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        if catalog_marker.as_deref() != Some("catalog-v3") {
            self.conn
                .execute_batch(include_str!("catalog-updates.sql"))?;
        }
        // Idempotent triage schema migration (v2026-05-11)
        let _ = self.conn.execute(
            "ALTER TABLE alerts ADD COLUMN status TEXT NOT NULL DEFAULT 'New'",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE alerts ADD COLUMN disposition TEXT NOT NULL DEFAULT 'Unknown'",
            [],
        );
        let _ = self
            .conn
            .execute("ALTER TABLE alerts ADD COLUMN severity_override TEXT", []);
        let _ = self
            .conn
            .execute("ALTER TABLE alerts ADD COLUMN confidence_score INTEGER", []);
        let _ = self
            .conn
            .execute("ALTER TABLE alerts ADD COLUMN owner TEXT", []);
        let _ = self
            .conn
            .execute("ALTER TABLE alerts ADD COLUMN triage_notes TEXT", []);
        let _ = self
            .conn
            .execute("ALTER TABLE alerts ADD COLUMN acknowledged_at TEXT", []);
        let _ = self
            .conn
            .execute("ALTER TABLE alerts ADD COLUMN investigating_at TEXT", []);
        let _ = self
            .conn
            .execute("ALTER TABLE alerts ADD COLUMN escalated_at TEXT", []);
        let _ = self
            .conn
            .execute("ALTER TABLE alerts ADD COLUMN closed_at TEXT", []);
        let _ = self
            .conn
            .execute("ALTER TABLE alerts ADD COLUMN closed_reason TEXT", []);
        let _ = self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_alerts_status ON alerts(status)",
            [],
        );
        let _ = self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_alerts_disposition ON alerts(disposition)",
            [],
        );
        let _ = self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_alerts_owner ON alerts(owner)",
            [],
        );
        let _ = self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_alerts_closed_at ON alerts(closed_at)",
            [],
        );
        let _ = self.conn.execute(
            "CREATE TABLE IF NOT EXISTS alert_triage_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                alert_id INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                old_value TEXT,
                new_value TEXT,
                note TEXT,
                actor TEXT NOT NULL DEFAULT 'local',
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY(alert_id) REFERENCES alerts(id) ON DELETE CASCADE
            )",
            [],
        );
        let _ = self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_triage_events_alert ON alert_triage_events(alert_id, created_at)", [],
        );
        // Idempotent feed fetch diagnostics migration (v2026-05-15)
        let _ = self.conn.execute(
            "ALTER TABLE feeds ADD COLUMN last_fetch_success_at TIMESTAMP",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE feeds ADD COLUMN last_fetch_failed_at TIMESTAMP",
            [],
        );
        let _ = self
            .conn
            .execute("ALTER TABLE feeds ADD COLUMN last_failure_phase TEXT", []);
        let _ = self
            .conn
            .execute("ALTER TABLE feeds ADD COLUMN last_failure_kind TEXT", []);
        let _ = self
            .conn
            .execute("ALTER TABLE feeds ADD COLUMN last_http_status INTEGER", []);
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS feed_fetch_attempts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                feed_id INTEGER NOT NULL,
                attempted_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                success INTEGER NOT NULL,
                url TEXT NOT NULL,
                final_url TEXT,
                http_status INTEGER,
                elapsed_ms INTEGER NOT NULL,
                failure_phase TEXT,
                failure_kind TEXT,
                error_summary TEXT,
                error_detail TEXT,
                items_seen INTEGER,
                items_new INTEGER,
                FOREIGN KEY(feed_id) REFERENCES feeds(id) ON DELETE CASCADE
            )",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_feed_fetch_attempts_feed
             ON feed_fetch_attempts(feed_id, attempted_at DESC)",
            [],
        )?;
        Ok(())
    }

    fn is_first_run_database(&self) -> Result<bool> {
        let seed_marker: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM app_meta WHERE key = 'first_run_seed_version'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        if seed_marker.is_some() {
            return Ok(false);
        }

        let feed_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM feeds", [], |row| row.get(0))?;
        let keyword_count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM keywords", [], |row| row.get(0))?;
        let alert_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM alerts", [], |row| row.get(0))?;
        Ok(feed_count == 0 && keyword_count == 0 && alert_count == 0)
    }

    fn ensure_builtin_enrichment_providers(&self) -> Result<()> {
        self.create_enrichment_provider_if_missing(&EnrichmentProviderCreate {
            name: "cisa-kev".into(),
            provider_type: "cisa_kev".into(),
            enabled: true,
            config_json: None,
            secret_ref: None,
            rate_limit_per_minute: None,
            supports_types: vec![IndicatorType::Cve],
        })?;
        self.create_enrichment_provider_if_missing(&EnrichmentProviderCreate {
            name: "urlhaus".into(),
            provider_type: "urlhaus".into(),
            enabled: false,
            config_json: Some(r#"{"cache_ttl_hours":12}"#.into()),
            secret_ref: Some("env:URLHAUS_AUTH_KEY".into()),
            rate_limit_per_minute: Some(30),
            supports_types: vec![
                IndicatorType::Url,
                IndicatorType::Domain,
                IndicatorType::Ipv4,
                IndicatorType::Md5,
                IndicatorType::Sha256,
            ],
        })?;
        Ok(())
    }

    // ── Feeds ─────────────────────────────────────────────────────────────

    pub fn create_feed(&self, feed: &FeedCreate) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO feeds (name, url, feed_type, enabled, interval_secs, api_template_id, api_key, custom_headers, tor_proxy)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                feed.name, feed.url, format!("{:?}", feed.feed_type),
                feed.enabled as i64, feed.interval_secs as i64,
                feed.api_template_id, feed.api_key, feed.custom_headers, feed.tor_proxy
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_feed(&self, id: i64) -> Result<Option<Feed>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, url, feed_type, enabled, interval_secs, last_fetch_at, last_error,
                    last_fetch_success_at, last_fetch_failed_at, last_failure_phase, last_failure_kind,
                    last_http_status, consecutive_failures, content_hash, created_at, api_template_id,
                    api_key, custom_headers, tor_proxy
             FROM feeds WHERE id = ?1"
        )?;
        stmt.query_row([id], Self::row_to_feed)
            .optional()
            .map_err(Into::into)
    }

    pub fn list_feeds(&self, filter: Option<&str>) -> Result<Vec<Feed>> {
        let has_filter = filter.map(|f| !f.is_empty()).unwrap_or(false);
        let sql = if has_filter {
            "SELECT id, name, url, feed_type, enabled, interval_secs, last_fetch_at, last_error,
                    last_fetch_success_at, last_fetch_failed_at, last_failure_phase, last_failure_kind,
                    last_http_status, consecutive_failures, content_hash, created_at, api_template_id,
                    api_key, custom_headers, tor_proxy
             FROM feeds WHERE name LIKE ?1 OR url LIKE ?1 ORDER BY id"
        } else {
            "SELECT id, name, url, feed_type, enabled, interval_secs, last_fetch_at, last_error,
                    last_fetch_success_at, last_fetch_failed_at, last_failure_phase, last_failure_kind,
                    last_http_status, consecutive_failures, content_hash, created_at, api_template_id,
                    api_key, custom_headers, tor_proxy
             FROM feeds ORDER BY id"
        };
        let mut stmt = self.conn.prepare(sql)?;
        let rows = if has_filter {
            stmt.query_map([format!("%{}%", filter.unwrap())], Self::row_to_feed)?
        } else {
            stmt.query_map([], Self::row_to_feed)?
        };
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn update_feed(&self, id: i64, feed: &FeedUpdate) -> Result<()> {
        self.conn.execute(
            "UPDATE feeds SET
                name = COALESCE(?1, name),
                url = COALESCE(?2, url),
                feed_type = COALESCE(?3, feed_type),
                enabled = COALESCE(?4, enabled),
                interval_secs = COALESCE(?5, interval_secs),
                api_template_id = ?6,
                api_key = ?7,
                custom_headers = ?8,
                tor_proxy = ?9
             WHERE id = ?10",
            params![
                feed.name.as_ref(),
                feed.url.as_ref(),
                feed.feed_type.map(|t| format!("{:?}", t)),
                feed.enabled.map(|e| e as i64),
                feed.interval_secs.map(|i| i as i64),
                feed.api_template_id,
                feed.api_key.as_ref(),
                feed.custom_headers.as_ref(),
                feed.tor_proxy.as_ref(),
                id
            ],
        )?;
        Ok(())
    }

    pub fn delete_feed(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM feeds WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn update_feed_health(
        &self,
        id: i64,
        success: bool,
        error: Option<&str>,
        content_hash: Option<&str>,
    ) -> Result<()> {
        if success {
            self.conn.execute(
                "UPDATE feeds SET consecutive_failures = 0, last_error = NULL, content_hash = ?1, last_fetch_at = CURRENT_TIMESTAMP WHERE id = ?2",
                params![content_hash, id],
            )?;
        } else {
            self.conn.execute(
                "UPDATE feeds SET consecutive_failures = consecutive_failures + 1, last_error = ?1, last_fetch_at = CURRENT_TIMESTAMP WHERE id = ?2",
                params![error, id],
            )?;
        }
        Ok(())
    }

    pub fn reset_feed_failures(&self, id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE feeds SET consecutive_failures = 0, last_error = NULL WHERE id = ?1",
            [id],
        )?;
        Ok(())
    }

    pub fn toggle_feed_enabled(&self, id: i64) -> Result<()> {
        self.conn
            .execute("UPDATE feeds SET enabled = NOT enabled WHERE id = ?1", [id])?;
        Ok(())
    }

    fn row_to_feed(row: &rusqlite::Row) -> rusqlite::Result<Feed> {
        let feed_type_str: String = row.get(3)?;
        let last_fetch: Option<String> = row.get(6)?;
        let last_success: Option<String> = row.get(8)?;
        let last_failed: Option<String> = row.get(9)?;
        let created: String = row.get(15)?;
        Ok(Feed {
            id: row.get(0)?,
            name: row.get(1)?,
            url: row.get(2)?,
            feed_type: FeedType::from(feed_type_str.as_str()),
            enabled: row.get::<_, i64>(4)? != 0,
            interval_secs: row.get::<_, i64>(5)? as u64,
            last_fetch_at: last_fetch.and_then(|s| parse_ts(&s)),
            last_error: row.get(7)?,
            last_fetch_success_at: last_success.and_then(|s| parse_ts(&s)),
            last_fetch_failed_at: last_failed.and_then(|s| parse_ts(&s)),
            last_failure_phase: row.get(10)?,
            last_failure_kind: row.get(11)?,
            last_http_status: row.get::<_, Option<i64>>(12)?.map(|status| status as u16),
            consecutive_failures: row.get::<_, i64>(13)? as u32,
            content_hash: row.get(14)?,
            created_at: parse_ts(&created).unwrap_or_else(Utc::now),
            api_template_id: row.get(16)?,
            api_key: row.get(17)?,
            custom_headers: row.get(18)?,
            tor_proxy: row.get(19)?,
        })
    }

    // ── Feed Items ────────────────────────────────────────────────────────

    pub fn upsert_feed_item(&self, item: &NewFeedItem) -> Result<i64> {
        self.conn.execute(
            "INSERT OR IGNORE INTO feed_items
             (feed_id, title, url, author, summary, content, published_at, content_hash, metadata_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                item.feed_id,
                item.title,
                item.url,
                item.author,
                item.summary,
                item.content,
                item.published_at.map(|dt| dt.to_rfc3339()),
                item.content_hash,
                item.metadata_json,
            ],
        )?;
        self.conn
            .query_row(
                "SELECT id FROM feed_items WHERE content_hash = ?1",
                [&item.content_hash],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn list_feed_items(&self, filter: &FeedItemFilter) -> Result<Vec<FeedItemWithFeed>> {
        let mut clauses = Vec::new();
        let mut values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(feed_id) = filter.feed_id {
            clauses.push("fi.feed_id = ?".to_string());
            values.push(Box::new(feed_id));
        }
        if filter.unread_only {
            clauses.push("fi.read = 0".to_string());
        }
        if let Some(text) = filter.text.as_ref().filter(|text| !text.is_empty()) {
            clauses.push("(fi.title LIKE ? OR COALESCE(fi.summary, '') LIKE ? OR COALESCE(fi.content, '') LIKE ? OR COALESCE(fi.url, '') LIKE ? OR f.name LIKE ?)".to_string());
            let pattern = format!("%{}%", text);
            for _ in 0..5 {
                values.push(Box::new(pattern.clone()));
            }
        }

        let where_sql = if clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", clauses.join(" AND "))
        };
        let limit_sql = filter
            .limit
            .map(|limit| format!(" LIMIT {}", limit))
            .unwrap_or_default();
        let sql = format!(
            "SELECT fi.id, fi.feed_id, fi.title, fi.url, fi.author, fi.summary, fi.content,
                    fi.published_at, fi.fetched_at, fi.content_hash, fi.read, fi.metadata_json,
                    f.name
             FROM feed_items fi
             JOIN feeds f ON fi.feed_id = f.id
             {}
             ORDER BY COALESCE(fi.published_at, fi.fetched_at) DESC, fi.id DESC{}",
            where_sql, limit_sql
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let refs: Vec<&dyn rusqlite::ToSql> = values.iter().map(|value| value.as_ref()).collect();
        let rows = stmt.query_map(
            rusqlite::params_from_iter(refs),
            Self::row_to_feed_item_with_feed,
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_feed_item(&self, id: i64) -> Result<Option<FeedItemWithFeed>> {
        let mut stmt = self.conn.prepare(
            "SELECT fi.id, fi.feed_id, fi.title, fi.url, fi.author, fi.summary, fi.content,
                    fi.published_at, fi.fetched_at, fi.content_hash, fi.read, fi.metadata_json,
                    f.name
             FROM feed_items fi
             JOIN feeds f ON fi.feed_id = f.id
             WHERE fi.id = ?1",
        )?;
        stmt.query_row([id], Self::row_to_feed_item_with_feed)
            .optional()
            .map_err(Into::into)
    }

    pub fn mark_feed_item_read(&self, id: i64, read: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE feed_items SET read = ?1 WHERE id = ?2",
            params![read as i64, id],
        )?;
        Ok(())
    }

    pub fn cache_feed_item_content(&self, id: i64, content: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE feed_items SET content = ?1 WHERE id = ?2",
            params![content, id],
        )?;
        Ok(())
    }

    pub fn store_feed_result_items(&self, feed: &Feed, result: &FeedResult) -> Result<usize> {
        Ok(self
            .store_feed_result_items_with_ids(feed, result)?
            .into_iter()
            .filter(|stored| stored.inserted)
            .count())
    }

    pub fn store_feed_result_items_with_ids(
        &self,
        feed: &Feed,
        result: &FeedResult,
    ) -> Result<Vec<StoredFeedItem>> {
        let mut stored_items = Vec::with_capacity(result.items.len());
        for item in &result.items {
            let new_item = NewFeedItem {
                feed_id: feed.id,
                title: item
                    .title
                    .clone()
                    .or_else(|| item.url.clone())
                    .unwrap_or_else(|| "Untitled article".to_string()),
                url: item.url.clone(),
                author: item.source.clone(),
                summary: item.description.clone(),
                content: None,
                published_at: item.date,
                content_hash: feed_item_hash(feed, item),
                metadata_json: item.raw_json.clone(),
            };
            let existed = self
                .conn
                .query_row(
                    "SELECT 1 FROM feed_items WHERE content_hash = ?1",
                    [&new_item.content_hash],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            let id = self.upsert_feed_item(&new_item)?;
            stored_items.push(StoredFeedItem {
                id,
                inserted: !existed,
            });
        }
        Ok(stored_items)
    }

    pub fn record_feed_fetch_attempt(
        &self,
        feed_id: i64,
        attempt: &crate::feed::diagnostics::FetchAttempt,
    ) -> Result<i64> {
        let diagnostic = attempt.diagnostic.as_ref();
        self.conn.execute(
            "INSERT INTO feed_fetch_attempts
             (feed_id, success, url, final_url, http_status, elapsed_ms,
              failure_phase, failure_kind, error_summary, error_detail, items_seen, items_new)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                feed_id,
                attempt.success as i64,
                attempt.url,
                attempt.final_url,
                attempt.http_status.map(|status| status as i64),
                attempt.elapsed_ms as i64,
                diagnostic.map(|d| d.phase.label()),
                diagnostic.map(|d| d.kind.label()),
                diagnostic.map(|d| d.summary.as_str()),
                diagnostic.and_then(|d| d.detail.as_deref()),
                attempt.items_seen.map(|value| value as i64),
                attempt.items_new.map(|value| value as i64),
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_feed_fetch_attempts(
        &self,
        feed_id: i64,
        limit: usize,
    ) -> Result<Vec<crate::feed::diagnostics::FetchAttempt>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, feed_id, attempted_at, success, url, final_url, http_status, elapsed_ms,
                    failure_phase, failure_kind, error_summary, error_detail, items_seen, items_new
             FROM feed_fetch_attempts
             WHERE feed_id = ?1
             ORDER BY attempted_at DESC, id DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![feed_id, limit as i64], Self::row_to_fetch_attempt)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn record_feed_fetch_outcome(
        &self,
        feed_id: i64,
        attempt: &crate::feed::diagnostics::FetchAttempt,
        content_hash: Option<&str>,
    ) -> Result<i64> {
        let attempt_id = self.record_feed_fetch_attempt(feed_id, attempt)?;
        let diagnostic = attempt.diagnostic.as_ref();

        if attempt.success {
            self.conn.execute(
                "UPDATE feeds SET
                    consecutive_failures = 0,
                    last_error = NULL,
                    last_failure_phase = NULL,
                    last_failure_kind = NULL,
                    last_http_status = NULL,
                    content_hash = ?1,
                    last_fetch_at = CURRENT_TIMESTAMP,
                    last_fetch_success_at = CURRENT_TIMESTAMP
                 WHERE id = ?2",
                params![content_hash, feed_id],
            )?;
        } else {
            self.conn.execute(
                "UPDATE feeds SET
                    consecutive_failures = consecutive_failures + 1,
                    last_error = ?1,
                    last_failure_phase = ?2,
                    last_failure_kind = ?3,
                    last_http_status = ?4,
                    last_fetch_at = CURRENT_TIMESTAMP,
                    last_fetch_failed_at = CURRENT_TIMESTAMP
                 WHERE id = ?5",
                params![
                    diagnostic.map(|d| d.summary.as_str()),
                    diagnostic.map(|d| d.phase.label()),
                    diagnostic.map(|d| d.kind.label()),
                    attempt.http_status.map(|status| status as i64),
                    feed_id,
                ],
            )?;
        }

        Ok(attempt_id)
    }

    fn row_to_fetch_attempt(
        row: &rusqlite::Row,
    ) -> rusqlite::Result<crate::feed::diagnostics::FetchAttempt> {
        use crate::feed::diagnostics::{
            FetchAttempt, FetchDiagnostic, FetchFailureKind, FetchFailurePhase,
        };

        let attempted_at: String = row.get(2)?;
        let url: String = row.get(4)?;
        let final_url: Option<String> = row.get(5)?;
        let http_status = row.get::<_, Option<i64>>(6)?.map(|status| status as u16);
        let elapsed_ms = row.get::<_, i64>(7)? as u128;
        let failure_phase: Option<String> = row.get(8)?;
        let failure_kind: Option<String> = row.get(9)?;
        let error_summary: Option<String> = row.get(10)?;
        let error_detail: Option<String> = row.get(11)?;

        let diagnostic = error_summary.map(|summary| FetchDiagnostic {
            phase: failure_phase
                .as_deref()
                .map(FetchFailurePhase::from_label)
                .unwrap_or(FetchFailurePhase::Unknown),
            kind: failure_kind
                .as_deref()
                .map(FetchFailureKind::from_label)
                .unwrap_or(FetchFailureKind::Unknown),
            summary,
            detail: error_detail,
            http_status,
            url: url.clone(),
            final_url: final_url.clone(),
            elapsed_ms,
        });

        Ok(FetchAttempt {
            id: row.get(0)?,
            feed_id: row.get(1)?,
            attempted_at: parse_db_datetime(&attempted_at),
            success: row.get::<_, i64>(3)? != 0,
            url,
            final_url,
            http_status,
            elapsed_ms,
            diagnostic,
            items_seen: row.get::<_, Option<i64>>(12)?.map(|value| value as usize),
            items_new: row.get::<_, Option<i64>>(13)?.map(|value| value as usize),
        })
    }

    fn row_to_feed_item_with_feed(row: &rusqlite::Row) -> rusqlite::Result<FeedItemWithFeed> {
        let published: Option<String> = row.get(7)?;
        let fetched: String = row.get(8)?;
        Ok(FeedItemWithFeed {
            item: FeedItem {
                id: row.get(0)?,
                feed_id: row.get(1)?,
                title: row.get(2)?,
                url: row.get(3)?,
                author: row.get(4)?,
                summary: row.get(5)?,
                content: row.get(6)?,
                published_at: published.and_then(|value| parse_db_datetime(&value)),
                fetched_at: parse_db_datetime(&fetched).unwrap_or_else(Utc::now),
                content_hash: row.get(9)?,
                read: row.get::<_, i64>(10)? != 0,
                metadata_json: row.get(11)?,
            },
            feed_name: row.get(12)?,
        })
    }

    // ── Templates ─────────────────────────────────────────────────────────

    pub fn create_template(&self, tmpl: &ApiTemplateCreate) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO api_templates (name, jsonpath_title, jsonpath_description, jsonpath_date, jsonpath_url, jsonpath_source, pagination_config)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                tmpl.name, tmpl.jsonpath_title, tmpl.jsonpath_description,
                tmpl.jsonpath_date, tmpl.jsonpath_url, tmpl.jsonpath_source, tmpl.pagination_config
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_template(&self, id: i64) -> Result<Option<ApiTemplate>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, jsonpath_title, jsonpath_description, jsonpath_date, jsonpath_url, jsonpath_source, pagination_config, created_at
             FROM api_templates WHERE id = ?1"
        )?;
        stmt.query_row([id], Self::row_to_template)
            .optional()
            .map_err(Into::into)
    }

    pub fn get_template_by_name(&self, name: &str) -> Result<Option<ApiTemplate>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, jsonpath_title, jsonpath_description, jsonpath_date, jsonpath_url, jsonpath_source, pagination_config, created_at
             FROM api_templates WHERE name = ?1"
        )?;
        stmt.query_row([name], Self::row_to_template)
            .optional()
            .map_err(Into::into)
    }

    pub fn list_templates(&self) -> Result<Vec<ApiTemplate>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, jsonpath_title, jsonpath_description, jsonpath_date, jsonpath_url, jsonpath_source, pagination_config, created_at
             FROM api_templates ORDER BY name"
        )?;
        let rows = stmt.query_map([], Self::row_to_template)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn row_to_template(row: &rusqlite::Row) -> rusqlite::Result<ApiTemplate> {
        let created: String = row.get(8)?;
        Ok(ApiTemplate {
            id: row.get(0)?,
            name: row.get(1)?,
            jsonpath_title: row.get(2)?,
            jsonpath_description: row.get(3)?,
            jsonpath_date: row.get(4)?,
            jsonpath_url: row.get(5)?,
            jsonpath_source: row.get(6)?,
            pagination_config: row.get(7)?,
            created_at: parse_ts(&created).unwrap_or_else(Utc::now),
        })
    }

    // ── Keywords ────────────────────────────────────────────────────────────

    pub fn create_keyword(&self, kw: &KeywordCreate) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO keywords (pattern, is_regex, case_sensitive, criticality, enabled)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                kw.pattern,
                kw.is_regex as i64,
                kw.case_sensitive as i64,
                format!("{:?}", kw.criticality),
                kw.enabled as i64
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_keyword(&self, id: i64) -> Result<Option<Keyword>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, pattern, is_regex, case_sensitive, criticality, enabled, created_at FROM keywords WHERE id = ?1"
        )?;
        stmt.query_row([id], Self::row_to_keyword)
            .optional()
            .map_err(Into::into)
    }

    pub fn list_keywords(&self, enabled_only: bool) -> Result<Vec<Keyword>> {
        let sql = if enabled_only {
            "SELECT id, pattern, is_regex, case_sensitive, criticality, enabled, created_at FROM keywords WHERE enabled = 1 ORDER BY id"
        } else {
            "SELECT id, pattern, is_regex, case_sensitive, criticality, enabled, created_at FROM keywords ORDER BY id"
        };
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map([], Self::row_to_keyword)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn update_keyword(&self, id: i64, kw: &KeywordUpdate) -> Result<()> {
        self.conn.execute(
            "UPDATE keywords SET
                pattern = COALESCE(?1, pattern),
                is_regex = COALESCE(?2, is_regex),
                case_sensitive = COALESCE(?3, case_sensitive),
                criticality = COALESCE(?4, criticality),
                enabled = COALESCE(?5, enabled)
             WHERE id = ?6",
            params![
                kw.pattern.as_ref(),
                kw.is_regex.map(|v| v as i64),
                kw.case_sensitive.map(|v| v as i64),
                kw.criticality.map(|c| format!("{:?}", c)),
                kw.enabled.map(|v| v as i64),
                id
            ],
        )?;
        Ok(())
    }

    pub fn delete_keyword(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM keywords WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn toggle_keyword_enabled(&self, id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE keywords SET enabled = NOT enabled WHERE id = ?1",
            [id],
        )?;
        Ok(())
    }

    fn row_to_keyword(row: &rusqlite::Row) -> rusqlite::Result<Keyword> {
        let criticality_str: String = row.get(4)?;
        let created: String = row.get(6)?;
        Ok(Keyword {
            id: row.get(0)?,
            pattern: row.get(1)?,
            is_regex: row.get::<_, i64>(2)? != 0,
            case_sensitive: row.get::<_, i64>(3)? != 0,
            criticality: Criticality::from(criticality_str.as_str()),
            enabled: row.get::<_, i64>(5)? != 0,
            created_at: parse_ts(&created).unwrap_or_else(Utc::now),
        })
    }

    // ── Alerts ────────────────────────────────────────────────────────────

    pub fn create_alert(&self, alert: &AlertCreate) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO alerts (feed_id, keyword_id, title, content_snippet, criticality, content_hash, metadata_json, status, disposition)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'New', 'Unknown')",
            params![
                alert.feed_id, alert.keyword_id, alert.title, alert.content_snippet,
                format!("{:?}", alert.criticality), alert.content_hash, alert.metadata_json
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_alert(&self, id: i64) -> Result<Option<Alert>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, feed_id, keyword_id, title, content_snippet, criticality, read, content_hash, detected_at, metadata_json,
                    status, disposition, severity_override, confidence_score, owner, triage_notes,
                    acknowledged_at, investigating_at, escalated_at, closed_at, closed_reason
             FROM alerts WHERE id = ?1"
        )?;
        stmt.query_row([id], Self::row_to_alert)
            .optional()
            .map_err(Into::into)
    }

    pub fn list_alerts(&self, filter: &AlertFilter) -> Result<Vec<AlertWithMeta>> {
        let mut conditions = Vec::new();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(crit) = &filter.criticality {
            params_vec.push(Box::new(format!("{:?}", crit)));
            conditions.push(format!("a.criticality = ?{}", params_vec.len()));
        }
        if filter.unread_only {
            conditions.push("a.read = 0".to_string());
        }
        if let Some(tag_id) = filter.tag_id {
            params_vec.push(Box::new(tag_id));
            conditions.push(format!(
                "a.id IN (SELECT alert_id FROM alert_tags WHERE tag_id = ?{})",
                params_vec.len()
            ));
        }
        if let Some(feed_id) = filter.feed_id {
            params_vec.push(Box::new(feed_id));
            conditions.push(format!("a.feed_id = ?{}", params_vec.len()));
        }
        if let Some(keyword_id) = filter.keyword_id {
            params_vec.push(Box::new(keyword_id));
            conditions.push(format!("a.keyword_id = ?{}", params_vec.len()));
        }
        if let Some(text) = &filter.text {
            if !text.is_empty() {
                params_vec.push(Box::new(format!("%{}%", text)));
                let idx = params_vec.len();
                conditions.push(format!(
                    "(a.content_snippet LIKE ?{idx} OR a.title LIKE ?{idx} OR f.name LIKE ?{idx} OR k.pattern LIKE ?{idx})"
                ));
            }
        }
        if let Some(status) = &filter.status {
            params_vec.push(Box::new(format!("{:?}", status)));
            conditions.push(format!("a.status = ?{}", params_vec.len()));
        }
        if let Some(disposition) = &filter.disposition {
            params_vec.push(Box::new(format!("{:?}", disposition)));
            conditions.push(format!("a.disposition = ?{}", params_vec.len()));
        }
        if let Some(owner) = &filter.owner {
            if !owner.is_empty() {
                params_vec.push(Box::new(format!("%{}%", owner)));
                conditions.push(format!("a.owner LIKE ?{}", params_vec.len()));
            }
        }
        if filter.open_only {
            conditions.push("a.status != 'Closed'".to_string());
        }
        if filter.closed_only {
            conditions.push("a.status = 'Closed'".to_string());
        }

        let where_clause = if conditions.is_empty() {
            "".to_string()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let limit = filter.limit.unwrap_or(500);
        params_vec.push(Box::new(limit));
        let limit_idx = params_vec.len();

        let sql = format!(
            "SELECT a.id, a.feed_id, a.keyword_id, a.title, a.content_snippet, a.criticality, a.read, a.content_hash, a.detected_at, a.metadata_json,
                    a.status, a.disposition, a.severity_override, a.confidence_score, a.owner, a.triage_notes,
                    a.acknowledged_at, a.investigating_at, a.escalated_at, a.closed_at, a.closed_reason,
                    f.name as feed_name, k.pattern as keyword_pattern
             FROM alerts a
             JOIN feeds f ON a.feed_id = f.id
             JOIN keywords k ON a.keyword_id = k.id
             {} ORDER BY a.detected_at DESC LIMIT ?{}",
            where_clause, limit_idx
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(
            rusqlite::params_from_iter(refs),
            Self::row_to_alert_with_meta,
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn mark_alert_read(&self, id: i64, read: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE alerts SET read = ?1 WHERE id = ?2",
            params![read as i64, id],
        )?;
        Ok(())
    }

    pub fn mark_all_alerts_read(&self, read: bool) -> Result<()> {
        self.conn
            .execute("UPDATE alerts SET read = ?1", [read as i64])?;
        Ok(())
    }

    pub fn delete_alert(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM alerts WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn delete_alerts_by_ids(&self, ids: &[i64]) -> Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }
        let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "DELETE FROM alerts WHERE id IN ({})",
            placeholders.join(",")
        );
        let count = self.conn.execute(&sql, rusqlite::params_from_iter(ids))?;
        Ok(count as u64)
    }

    pub fn delete_old_alerts(&self, before: DateTime<Utc>) -> Result<u64> {
        let count = self.conn.execute(
            "DELETE FROM alerts WHERE detected_at < ?1",
            [before.to_rfc3339()],
        )?;
        Ok(count as u64)
    }

    pub fn count_old_alerts(&self, before: DateTime<Utc>) -> Result<u64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM alerts WHERE detected_at < ?1",
            [before.to_rfc3339()],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    pub fn get_alert_count(&self) -> Result<i64> {
        let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM alerts")?;
        let count: i64 = stmt.query_row([], |row| row.get(0))?;
        Ok(count)
    }

    pub fn get_feed_count(&self) -> Result<i64> {
        let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM feeds")?;
        let count: i64 = stmt.query_row([], |row| row.get(0))?;
        Ok(count)
    }

    pub fn get_unread_alert_count(&self) -> Result<i64> {
        let mut stmt = self
            .conn
            .prepare("SELECT COUNT(*) FROM alerts WHERE read = 0")?;
        let count: i64 = stmt.query_row([], |row| row.get(0))?;
        Ok(count)
    }

    pub fn alert_exists_by_hash_window(&self, hash: &str, window: Duration) -> Result<bool> {
        let since = (Utc::now() - window).to_rfc3339();
        let mut stmt = self
            .conn
            .prepare("SELECT 1 FROM alerts WHERE content_hash = ?1 AND detected_at > ?2 LIMIT 1")?;
        let exists = stmt
            .query_row(params![hash, since], |_row| Ok(()))
            .optional()?
            .is_some();
        Ok(exists)
    }

    pub fn get_criticality_distribution(&self) -> Result<Vec<(Criticality, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT criticality, COUNT(*) FROM alerts GROUP BY criticality ORDER BY criticality",
        )?;
        let rows = stmt.query_map([], |row| {
            let crit_str: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            Ok((Criticality::from(crit_str.as_str()), count))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_top_keywords(&self, limit: usize) -> Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT k.pattern, COUNT(*) as cnt FROM alerts a JOIN keywords k ON a.keyword_id = k.id
             GROUP BY a.keyword_id ORDER BY cnt DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_alert_trend(&self, days: u32) -> Result<Vec<(String, i64)>> {
        let since = (Utc::now() - Duration::days(days as i64))
            .format("%Y-%m-%d")
            .to_string();
        let mut stmt = self.conn.prepare(
            "SELECT DATE(detected_at) as day, COUNT(*) as cnt FROM alerts
             WHERE detected_at > ?1 GROUP BY day ORDER BY day",
        )?;
        let rows = stmt.query_map([since], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn row_to_alert(row: &rusqlite::Row) -> rusqlite::Result<Alert> {
        let criticality_str: String = row.get(5)?;
        let detected_str: String = row.get(8)?;
        let status_str: String = row.get(10)?;
        let disposition_str: String = row.get(11)?;
        let severity_override_str: Option<String> = row.get(12)?;
        let acknowledged_str: Option<String> = row.get(16)?;
        let investigating_str: Option<String> = row.get(17)?;
        let escalated_str: Option<String> = row.get(18)?;
        let closed_str: Option<String> = row.get(19)?;
        Ok(Alert {
            id: row.get(0)?,
            feed_id: row.get(1)?,
            keyword_id: row.get(2)?,
            title: row.get(3)?,
            content_snippet: row.get(4)?,
            criticality: Criticality::from(criticality_str.as_str()),
            read: row.get::<_, i64>(6)? != 0,
            content_hash: row.get(7)?,
            detected_at: parse_ts(&detected_str).unwrap_or_else(Utc::now),
            metadata_json: row.get(9)?,
            status: AlertStatus::from(status_str.as_str()),
            disposition: AlertDisposition::from(disposition_str.as_str()),
            severity_override: severity_override_str.as_deref().map(Criticality::from),
            confidence_score: row.get(13)?,
            owner: row.get(14)?,
            triage_notes: row.get(15)?,
            acknowledged_at: acknowledged_str.and_then(|s| parse_ts(&s)),
            investigating_at: investigating_str.and_then(|s| parse_ts(&s)),
            escalated_at: escalated_str.and_then(|s| parse_ts(&s)),
            closed_at: closed_str.and_then(|s| parse_ts(&s)),
            closed_reason: row.get(20)?,
        })
    }

    fn row_to_alert_with_meta(row: &rusqlite::Row) -> rusqlite::Result<AlertWithMeta> {
        let alert = Self::row_to_alert(row)?;
        let feed_name: String = row.get(21)?;
        let keyword_pattern: String = row.get(22)?;
        Ok(AlertWithMeta {
            alert,
            feed_name,
            keyword_pattern,
            tags: Vec::new(), // populated separately if needed
        })
    }

    // ── Alert Triage ──────────────────────────────────────────────────────

    pub fn update_alert_status(
        &self,
        alert_id: i64,
        status: AlertStatus,
        note: Option<&str>,
    ) -> Result<()> {
        let old_status: String = self.conn.query_row(
            "SELECT status FROM alerts WHERE id = ?1",
            [alert_id],
            |row| row.get(0),
        )?;
        let timestamp_col = match status {
            AlertStatus::Acknowledged => "acknowledged_at",
            AlertStatus::Investigating => "investigating_at",
            AlertStatus::Escalated => "escalated_at",
            _ => "",
        };
        let ts_sql = if timestamp_col.is_empty() {
            String::new()
        } else {
            format!(", {timestamp_col} = CASE WHEN {timestamp_col} IS NULL THEN CURRENT_TIMESTAMP ELSE {timestamp_col} END")
        };
        let sql = format!("UPDATE alerts SET status = ?1{ts_sql} WHERE id = ?2");
        self.conn
            .execute(&sql, params![format!("{:?}", status), alert_id])?;
        self.insert_triage_event(
            alert_id,
            "status_changed",
            Some(&old_status),
            Some(&format!("{:?}", status)),
            note,
        )?;
        Ok(())
    }

    pub fn update_alert_disposition(
        &self,
        alert_id: i64,
        disposition: AlertDisposition,
        note: Option<&str>,
    ) -> Result<()> {
        let old_disp: String = self.conn.query_row(
            "SELECT disposition FROM alerts WHERE id = ?1",
            [alert_id],
            |row| row.get(0),
        )?;
        self.conn.execute(
            "UPDATE alerts SET disposition = ?1 WHERE id = ?2",
            params![format!("{:?}", disposition), alert_id],
        )?;
        self.insert_triage_event(
            alert_id,
            "disposition_changed",
            Some(&old_disp),
            Some(&format!("{:?}", disposition)),
            note,
        )?;
        Ok(())
    }

    pub fn update_alert_severity(
        &self,
        alert_id: i64,
        severity: Option<Criticality>,
        note: Option<&str>,
    ) -> Result<()> {
        let old_sev: Option<String> = self.conn.query_row(
            "SELECT severity_override FROM alerts WHERE id = ?1",
            [alert_id],
            |row| row.get(0),
        )?;
        self.conn.execute(
            "UPDATE alerts SET severity_override = ?1 WHERE id = ?2",
            params![severity.map(|c| format!("{:?}", c)), alert_id],
        )?;
        self.insert_triage_event(
            alert_id,
            "severity_changed",
            old_sev.as_deref(),
            severity.map(|c| format!("{:?}", c)).as_deref(),
            note,
        )?;
        Ok(())
    }

    pub fn update_alert_confidence(
        &self,
        alert_id: i64,
        confidence: Option<i64>,
        note: Option<&str>,
    ) -> Result<()> {
        let old_conf: Option<i64> = self.conn.query_row(
            "SELECT confidence_score FROM alerts WHERE id = ?1",
            [alert_id],
            |row| row.get(0),
        )?;
        self.conn.execute(
            "UPDATE alerts SET confidence_score = ?1 WHERE id = ?2",
            params![confidence, alert_id],
        )?;
        self.insert_triage_event(
            alert_id,
            "confidence_changed",
            old_conf.map(|c| c.to_string()).as_deref(),
            confidence.map(|c| c.to_string()).as_deref(),
            note,
        )?;
        Ok(())
    }

    pub fn assign_alert_owner(
        &self,
        alert_id: i64,
        owner: Option<&str>,
        note: Option<&str>,
    ) -> Result<()> {
        let old_owner: Option<String> = self.conn.query_row(
            "SELECT owner FROM alerts WHERE id = ?1",
            [alert_id],
            |row| row.get(0),
        )?;
        self.conn.execute(
            "UPDATE alerts SET owner = ?1 WHERE id = ?2",
            params![owner, alert_id],
        )?;
        self.insert_triage_event(alert_id, "owner_changed", old_owner.as_deref(), owner, note)?;
        Ok(())
    }

    pub fn add_alert_note(&self, alert_id: i64, note: &str) -> Result<()> {
        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT triage_notes FROM alerts WHERE id = ?1",
                [alert_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        let new_notes = match existing {
            Some(existing) if !existing.is_empty() => format!("{}\n{}", existing, note),
            _ => note.to_string(),
        };
        self.conn.execute(
            "UPDATE alerts SET triage_notes = ?1 WHERE id = ?2",
            params![new_notes, alert_id],
        )?;
        self.insert_triage_event(alert_id, "note_added", None, None, Some(note))?;
        Ok(())
    }

    pub fn close_alert(
        &self,
        alert_id: i64,
        disposition: AlertDisposition,
        reason: Option<&str>,
    ) -> Result<()> {
        let old_status: String = self.conn.query_row(
            "SELECT status FROM alerts WHERE id = ?1",
            [alert_id],
            |row| row.get(0),
        )?;
        self.conn.execute(
            "UPDATE alerts SET status = 'Closed', disposition = ?1, closed_at = CURRENT_TIMESTAMP, closed_reason = ?2 WHERE id = ?3",
            params![format!("{:?}", disposition), reason, alert_id],
        )?;
        self.insert_triage_event(
            alert_id,
            "closed",
            Some(&old_status),
            Some("Closed"),
            reason,
        )?;
        self.insert_triage_event(
            alert_id,
            "disposition_changed",
            None,
            Some(&format!("{:?}", disposition)),
            reason,
        )?;
        Ok(())
    }

    pub fn reopen_alert(&self, alert_id: i64, note: Option<&str>) -> Result<()> {
        let old_status: String = self.conn.query_row(
            "SELECT status FROM alerts WHERE id = ?1",
            [alert_id],
            |row| row.get(0),
        )?;
        self.conn.execute(
            "UPDATE alerts SET status = 'Acknowledged', closed_at = NULL, closed_reason = NULL WHERE id = ?1",
            [alert_id],
        )?;
        self.insert_triage_event(
            alert_id,
            "reopened",
            Some(&old_status),
            Some("Acknowledged"),
            note,
        )?;
        Ok(())
    }

    pub fn bulk_update_alert_status(
        &self,
        alert_ids: &[i64],
        status: AlertStatus,
        note: Option<&str>,
    ) -> Result<u64> {
        if alert_ids.is_empty() {
            return Ok(0);
        }
        let status_str = format!("{:?}", status);
        let placeholders: Vec<String> = alert_ids.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "UPDATE alerts SET status = ?1 WHERE id IN ({})",
            placeholders.join(",")
        );
        let mut params: Vec<&dyn rusqlite::ToSql> = vec![&status_str as &dyn rusqlite::ToSql];
        for id in alert_ids {
            params.push(id);
        }
        let count = self
            .conn
            .execute(&sql, rusqlite::params_from_iter(params))?;
        for alert_id in alert_ids {
            let _ = self.insert_triage_event(
                *alert_id,
                "status_changed",
                None,
                Some(&format!("{:?}", status)),
                note,
            );
        }
        Ok(count as u64)
    }

    pub fn list_alert_triage_events(&self, alert_id: i64) -> Result<Vec<AlertTriageEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, alert_id, event_type, old_value, new_value, note, actor, created_at
             FROM alert_triage_events
             WHERE alert_id = ?1
             ORDER BY created_at ASC, id ASC",
        )?;
        let rows = stmt.query_map([alert_id], Self::row_to_triage_event)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_alert_status_counts(&self) -> Result<HashMap<String, i64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT status, COUNT(*) FROM alerts GROUP BY status")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        rows.collect::<Result<HashMap<_, _>, _>>()
            .map_err(Into::into)
    }

    pub fn get_alert_disposition_counts(&self) -> Result<HashMap<String, i64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT disposition, COUNT(*) FROM alerts GROUP BY disposition")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        rows.collect::<Result<HashMap<_, _>, _>>()
            .map_err(Into::into)
    }

    fn insert_triage_event(
        &self,
        alert_id: i64,
        event_type: &str,
        old_value: Option<&str>,
        new_value: Option<&str>,
        note: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO alert_triage_events (alert_id, event_type, old_value, new_value, note, actor)
             VALUES (?1, ?2, ?3, ?4, ?5, 'local')",
            params![alert_id, event_type, old_value, new_value, note],
        )?;
        Ok(())
    }

    fn row_to_triage_event(row: &rusqlite::Row) -> rusqlite::Result<AlertTriageEvent> {
        let created_str: String = row.get(7)?;
        Ok(AlertTriageEvent {
            id: row.get(0)?,
            alert_id: row.get(1)?,
            event_type: row.get(2)?,
            old_value: row.get(3)?,
            new_value: row.get(4)?,
            note: row.get(5)?,
            actor: row.get(6)?,
            created_at: parse_ts(&created_str).unwrap_or_else(Utc::now),
        })
    }

    // ── Indicators ────────────────────────────────────────────────────────

    pub fn upsert_indicator(&self, indicator: &ExtractedIndicator) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO indicators
                (indicator_type, value, normalized_value, confidence_score)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(indicator_type, normalized_value) DO UPDATE SET
                value = excluded.value,
                last_seen_at = CURRENT_TIMESTAMP,
                sighting_count = sighting_count + 1,
                confidence_score = COALESCE(excluded.confidence_score, confidence_score),
                updated_at = CURRENT_TIMESTAMP",
            params![
                indicator_type_to_db(indicator.indicator_type),
                indicator.value,
                indicator.normalized_value,
                indicator.confidence_hint.map(i64::from),
            ],
        )?;

        self.conn
            .query_row(
                "SELECT id FROM indicators WHERE indicator_type = ?1 AND normalized_value = ?2",
                params![
                    indicator_type_to_db(indicator.indicator_type),
                    indicator.normalized_value
                ],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn insert_indicator_occurrence(
        &self,
        occurrence: &IndicatorOccurrenceCreate,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO indicator_occurrences
                (indicator_id, content_item_id, alert_id, feed_id, source_field, start_offset, end_offset, surrounding_text)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                occurrence.indicator_id,
                occurrence.content_item_id,
                occurrence.alert_id,
                occurrence.feed_id,
                occurrence.source_field,
                occurrence.start_offset,
                occurrence.end_offset,
                occurrence.surrounding_text,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn link_indicator_to_alert(&self, alert_id: i64, indicator_id: i64) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO alert_indicators (alert_id, indicator_id, relationship)
             VALUES (?1, ?2, 'observed')",
            params![alert_id, indicator_id],
        )?;
        Ok(())
    }

    pub fn store_extracted_indicators(
        &self,
        indicators: &[ExtractedIndicator],
        alert_id: Option<i64>,
        content_item_id: Option<i64>,
        feed_id: Option<i64>,
    ) -> Result<Vec<i64>> {
        let mut ids = Vec::with_capacity(indicators.len());
        for indicator in indicators {
            let indicator_id = self.upsert_indicator(indicator)?;
            self.insert_indicator_occurrence(&IndicatorOccurrenceCreate {
                indicator_id,
                content_item_id,
                alert_id,
                feed_id,
                source_field: Some(indicator.source_field.clone()),
                start_offset: Some(indicator.start_offset as i64),
                end_offset: Some(indicator.end_offset as i64),
                surrounding_text: Some(indicator.surrounding_text.clone()),
            })?;
            if let Some(alert_id) = alert_id {
                self.link_indicator_to_alert(alert_id, indicator_id)?;
            }
            ids.push(indicator_id);
        }
        Ok(ids)
    }

    pub fn list_indicators_for_alert(&self, alert_id: i64) -> Result<Vec<IndicatorRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT i.id, i.indicator_type, i.value, i.normalized_value, i.first_seen_at,
                    i.last_seen_at, i.sighting_count, i.confidence_score, i.risk_score,
                    i.metadata_json, i.created_at, i.updated_at
             FROM indicators i
             JOIN alert_indicators ai ON ai.indicator_id = i.id
             WHERE ai.alert_id = ?1
             ORDER BY i.indicator_type, i.normalized_value",
        )?;
        let rows = stmt.query_map([alert_id], Self::row_to_indicator)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn list_indicators_for_content_item(
        &self,
        content_item_id: i64,
    ) -> Result<Vec<IndicatorRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT i.id, i.indicator_type, i.value, i.normalized_value, i.first_seen_at,
                    i.last_seen_at, i.sighting_count, i.confidence_score, i.risk_score,
                    i.metadata_json, i.created_at, i.updated_at
             FROM indicators i
             JOIN indicator_occurrences io ON io.indicator_id = i.id
             WHERE io.content_item_id = ?1
             ORDER BY i.indicator_type, i.normalized_value",
        )?;
        let rows = stmt.query_map([content_item_id], Self::row_to_indicator)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_indicator_detail(&self, id: i64) -> Result<Option<IndicatorDetail>> {
        let indicator = self.get_indicator(id)?;
        let Some(indicator) = indicator else {
            return Ok(None);
        };
        let occurrences = self.list_indicator_occurrences(id)?;
        Ok(Some(IndicatorDetail {
            indicator,
            occurrences,
        }))
    }

    pub fn search_indicators(&self, search: &IndicatorSearch) -> Result<Vec<IndicatorRecord>> {
        let mut sql = "SELECT id, indicator_type, value, normalized_value, first_seen_at,
                    last_seen_at, sighting_count, confidence_score, risk_score,
                    metadata_json, created_at, updated_at
             FROM indicators"
            .to_string();
        let mut conditions = Vec::new();
        let mut values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(indicator_type) = search.indicator_type {
            conditions.push("indicator_type = ?".to_string());
            values.push(Box::new(indicator_type_to_db(indicator_type).to_string()));
        }
        if let Some(text) = &search.text {
            if !text.is_empty() {
                conditions.push("(value LIKE ? OR normalized_value LIKE ?)".to_string());
                let like = format!("%{text}%");
                values.push(Box::new(like.clone()));
                values.push(Box::new(like));
            }
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }
        sql.push_str(" ORDER BY last_seen_at DESC, id DESC LIMIT ?");
        values.push(Box::new(search.limit.unwrap_or(500)));

        let params = values
            .iter()
            .map(|value| value.as_ref())
            .collect::<Vec<&dyn rusqlite::ToSql>>();
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params.as_slice(), Self::row_to_indicator)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_indicator(&self, id: i64) -> Result<Option<IndicatorRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, indicator_type, value, normalized_value, first_seen_at,
                    last_seen_at, sighting_count, confidence_score, risk_score,
                    metadata_json, created_at, updated_at
             FROM indicators WHERE id = ?1",
        )?;
        stmt.query_row([id], Self::row_to_indicator)
            .optional()
            .map_err(Into::into)
    }

    fn list_indicator_occurrences(&self, indicator_id: i64) -> Result<Vec<IndicatorOccurrence>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, indicator_id, content_item_id, alert_id, feed_id, source_field,
                    start_offset, end_offset, surrounding_text, detected_at
             FROM indicator_occurrences
             WHERE indicator_id = ?1
             ORDER BY detected_at DESC, id DESC",
        )?;
        let rows = stmt.query_map([indicator_id], Self::row_to_indicator_occurrence)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn row_to_indicator(row: &rusqlite::Row) -> rusqlite::Result<IndicatorRecord> {
        let indicator_type: String = row.get(1)?;
        let first_seen_at: String = row.get(4)?;
        let last_seen_at: String = row.get(5)?;
        let created_at: String = row.get(10)?;
        let updated_at: String = row.get(11)?;
        Ok(IndicatorRecord {
            id: row.get(0)?,
            indicator_type: indicator_type_from_db(&indicator_type),
            value: row.get(2)?,
            normalized_value: row.get(3)?,
            first_seen_at: parse_db_datetime(&first_seen_at).unwrap_or_else(Utc::now),
            last_seen_at: parse_db_datetime(&last_seen_at).unwrap_or_else(Utc::now),
            sighting_count: row.get::<_, i64>(6)?,
            confidence_score: row.get(7)?,
            risk_score: row.get(8)?,
            metadata_json: row.get(9)?,
            created_at: parse_db_datetime(&created_at).unwrap_or_else(Utc::now),
            updated_at: parse_db_datetime(&updated_at).unwrap_or_else(Utc::now),
        })
    }

    fn row_to_indicator_occurrence(row: &rusqlite::Row) -> rusqlite::Result<IndicatorOccurrence> {
        let detected_at: String = row.get(9)?;
        Ok(IndicatorOccurrence {
            id: row.get(0)?,
            indicator_id: row.get(1)?,
            content_item_id: row.get(2)?,
            alert_id: row.get(3)?,
            feed_id: row.get(4)?,
            source_field: row.get(5)?,
            start_offset: row.get(6)?,
            end_offset: row.get(7)?,
            surrounding_text: row.get(8)?,
            detected_at: parse_db_datetime(&detected_at).unwrap_or_else(Utc::now),
        })
    }

    // ── Enrichment ────────────────────────────────────────────────────────

    pub fn create_enrichment_provider(&self, provider: &EnrichmentProviderCreate) -> Result<i64> {
        let supports_types_json = indicator_types_to_json(&provider.supports_types)?;
        self.conn.execute(
            "INSERT INTO enrichment_providers
                (name, provider_type, enabled, config_json, secret_ref, rate_limit_per_minute, supports_types_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(name) DO UPDATE SET
                provider_type = excluded.provider_type,
                enabled = excluded.enabled,
                config_json = excluded.config_json,
                secret_ref = excluded.secret_ref,
                rate_limit_per_minute = excluded.rate_limit_per_minute,
                supports_types_json = excluded.supports_types_json,
                updated_at = CURRENT_TIMESTAMP",
            params![
                provider.name,
                provider.provider_type,
                provider.enabled as i64,
                provider.config_json,
                provider.secret_ref,
                provider.rate_limit_per_minute.map(i64::from),
                supports_types_json,
            ],
        )?;
        self.conn
            .query_row(
                "SELECT id FROM enrichment_providers WHERE name = ?1",
                [provider.name.as_str()],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn create_enrichment_provider_if_missing(
        &self,
        provider: &EnrichmentProviderCreate,
    ) -> Result<i64> {
        if let Some(id) = self
            .conn
            .query_row(
                "SELECT id FROM enrichment_providers WHERE name = ?1",
                [provider.name.as_str()],
                |row| row.get(0),
            )
            .optional()?
        {
            return Ok(id);
        }

        self.create_enrichment_provider(provider)
    }

    pub fn update_enrichment_provider(&self, provider: &EnrichmentProviderUpdate) -> Result<()> {
        let supports_types_json = provider
            .supports_types
            .as_ref()
            .map(|types| indicator_types_to_json(types))
            .transpose()?;
        self.conn.execute(
            "UPDATE enrichment_providers SET
                enabled = COALESCE(?1, enabled),
                config_json = COALESCE(?2, config_json),
                secret_ref = COALESCE(?3, secret_ref),
                rate_limit_per_minute = COALESCE(?4, rate_limit_per_minute),
                supports_types_json = COALESCE(?5, supports_types_json),
                updated_at = CURRENT_TIMESTAMP
             WHERE id = ?6",
            params![
                provider.enabled.map(|enabled| enabled as i64),
                provider.config_json,
                provider.secret_ref,
                provider.rate_limit_per_minute.map(i64::from),
                supports_types_json,
                provider.id,
            ],
        )?;
        Ok(())
    }

    pub fn list_enabled_enrichment_providers(&self) -> Result<Vec<EnrichmentProviderRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, provider_type, enabled, config_json, secret_ref,
                    rate_limit_per_minute, supports_types_json, created_at, updated_at
             FROM enrichment_providers
             WHERE enabled = 1
             ORDER BY name",
        )?;
        let rows = stmt.query_map([], Self::row_to_enrichment_provider)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn list_enrichment_providers(&self) -> Result<Vec<EnrichmentProviderRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, provider_type, enabled, config_json, secret_ref,
                    rate_limit_per_minute, supports_types_json, created_at, updated_at
             FROM enrichment_providers
             ORDER BY name",
        )?;
        let rows = stmt.query_map([], Self::row_to_enrichment_provider)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn set_enrichment_provider_enabled(&self, name: &str, enabled: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE enrichment_providers SET enabled = ?1, updated_at = CURRENT_TIMESTAMP WHERE name = ?2",
            params![enabled as i64, name],
        )?;
        Ok(())
    }

    pub fn get_enrichment_provider(&self, id: i64) -> Result<Option<EnrichmentProviderRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, provider_type, enabled, config_json, secret_ref,
                    rate_limit_per_minute, supports_types_json, created_at, updated_at
             FROM enrichment_providers
             WHERE id = ?1",
        )?;
        stmt.query_row([id], Self::row_to_enrichment_provider)
            .optional()
            .map_err(Into::into)
    }

    pub fn queue_enrichment_job(
        &self,
        indicator_id: i64,
        provider_id: i64,
        priority: i64,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT OR IGNORE INTO enrichment_jobs
                (indicator_id, provider_id, status, priority)
             VALUES (?1, ?2, 'pending', ?3)",
            params![indicator_id, provider_id, priority],
        )?;
        self.conn
            .query_row(
                "SELECT id FROM enrichment_jobs WHERE indicator_id = ?1 AND provider_id = ?2",
                params![indicator_id, provider_id],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn queue_enrichment_jobs_for_indicators(&self, indicator_ids: &[i64]) -> Result<Vec<i64>> {
        let providers = self.list_enabled_enrichment_providers()?;
        let mut queued = Vec::new();
        for indicator_id in indicator_ids {
            let Some(indicator) = self.get_indicator(*indicator_id)? else {
                continue;
            };
            for provider in &providers {
                if !provider.supports_types.contains(&indicator.indicator_type) {
                    continue;
                }
                if self.has_fresh_enrichment_result(*indicator_id, provider.id)? {
                    continue;
                }
                queued.push(self.queue_enrichment_job(*indicator_id, provider.id, 100)?);
            }
        }
        Ok(queued)
    }

    pub fn has_fresh_enrichment_result(&self, indicator_id: i64, provider_id: i64) -> Result<bool> {
        let exists = self
            .conn
            .query_row(
                "SELECT 1 FROM enrichment_results
                 WHERE indicator_id = ?1
                   AND provider_id = ?2
                   AND status = 'succeeded'
                   AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)
                 LIMIT 1",
                params![indicator_id, provider_id],
                |_row| Ok(()),
            )
            .optional()?
            .is_some();
        Ok(exists)
    }

    pub fn claim_next_enrichment_jobs(&self, limit: i64) -> Result<Vec<EnrichmentJobRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id FROM enrichment_jobs
             WHERE status IN ('pending', 'retrying', 'rate_limited') AND next_attempt_at <= CURRENT_TIMESTAMP
             ORDER BY priority ASC, created_at ASC
             LIMIT ?1",
        )?;
        let job_ids = stmt
            .query_map([limit], |row| row.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        for job_id in &job_ids {
            self.conn.execute(
                "UPDATE enrichment_jobs SET
                    status = 'running',
                    last_attempt_at = CURRENT_TIMESTAMP,
                    updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?1",
                [job_id],
            )?;
        }

        let mut claimed = Vec::with_capacity(job_ids.len());
        for job_id in job_ids {
            if let Some(job) = self.get_enrichment_job(job_id)? {
                claimed.push(job);
            }
        }
        Ok(claimed)
    }

    pub fn mark_enrichment_job_succeeded(&self, job_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE enrichment_jobs SET
                status = 'succeeded',
                error_message = NULL,
                updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1",
            [job_id],
        )?;
        Ok(())
    }

    pub fn mark_enrichment_job_failed(
        &self,
        job_id: i64,
        error_message: &str,
        retry: bool,
    ) -> Result<()> {
        let status = if retry { "retrying" } else { "failed" };
        let next_attempt = if retry {
            "datetime(CURRENT_TIMESTAMP, '+5 minutes')"
        } else {
            "CURRENT_TIMESTAMP"
        };
        let sql = format!(
            "UPDATE enrichment_jobs SET
                status = ?1,
                attempt_count = attempt_count + 1,
                next_attempt_at = {next_attempt},
                error_message = ?2,
                updated_at = CURRENT_TIMESTAMP
             WHERE id = ?3"
        );
        self.conn
            .execute(&sql, params![status, error_message, job_id])?;
        Ok(())
    }

    pub fn mark_enrichment_job_rate_limited(&self, job_id: i64, error_message: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE enrichment_jobs SET
                status = 'rate_limited',
                next_attempt_at = datetime(CURRENT_TIMESTAMP, '+1 minutes'),
                error_message = ?1,
                updated_at = CURRENT_TIMESTAMP
             WHERE id = ?2",
            params![error_message, job_id],
        )?;
        Ok(())
    }

    pub fn store_enrichment_result(
        &self,
        indicator_id: i64,
        provider_id: i64,
        result: &sentinel_enrichment::EnrichmentResult,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO enrichment_results
                (indicator_id, provider_id, status, reputation, score, verdict, summary, raw_json, expires_at)
             VALUES (?1, ?2, 'succeeded', ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                indicator_id,
                provider_id,
                format!("{:?}", result.reputation),
                result.score,
                result.verdict,
                result.summary,
                result.raw_json.to_string(),
                result.expires_at.map(|dt| dt.to_rfc3339()),
            ],
        )?;
        if let Some(risk_score) = risk_score_from_enrichment(result) {
            self.update_indicator_risk_score(indicator_id, risk_score)?;
        }
        Ok(self.conn.last_insert_rowid())
    }

    fn update_indicator_risk_score(&self, indicator_id: i64, risk_score: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE indicators SET
                risk_score = CASE
                    WHEN risk_score IS NULL OR risk_score < ?1 THEN ?1
                    ELSE risk_score
                END,
                updated_at = CURRENT_TIMESTAMP
             WHERE id = ?2",
            params![risk_score, indicator_id],
        )?;
        Ok(())
    }

    pub fn get_latest_enrichment_results(
        &self,
        indicator_id: i64,
    ) -> Result<Vec<EnrichmentResultRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT er.id, er.indicator_id, er.provider_id, er.status, er.reputation,
                    er.score, er.verdict, er.summary, er.raw_json, er.fetched_at,
                    er.expires_at, er.created_at, er.updated_at
             FROM enrichment_results er
             JOIN (
                SELECT provider_id, MAX(fetched_at) AS fetched_at
                FROM enrichment_results
                WHERE indicator_id = ?1
                GROUP BY provider_id
             ) latest ON latest.provider_id = er.provider_id AND latest.fetched_at = er.fetched_at
             WHERE er.indicator_id = ?1
             ORDER BY er.fetched_at DESC",
        )?;
        let rows = stmt.query_map([indicator_id], Self::row_to_enrichment_result)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_enrichment_job(&self, id: i64) -> Result<Option<EnrichmentJobRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, indicator_id, provider_id, status, priority, attempt_count,
                    next_attempt_at, last_attempt_at, error_message, created_at, updated_at
             FROM enrichment_jobs
             WHERE id = ?1",
        )?;
        stmt.query_row([id], Self::row_to_enrichment_job)
            .optional()
            .map_err(Into::into)
    }

    pub fn list_enrichment_jobs(&self, limit: i64) -> Result<Vec<EnrichmentJobWithContext>> {
        let mut stmt = self.conn.prepare(
            "SELECT ej.id, ej.indicator_id, ej.provider_id, ej.status, ej.priority,
                    ej.attempt_count, ej.next_attempt_at, ej.last_attempt_at,
                    ej.error_message, ej.created_at, ej.updated_at,
                    ep.name, ep.provider_type, i.indicator_type, i.normalized_value
             FROM enrichment_jobs ej
             JOIN enrichment_providers ep ON ep.id = ej.provider_id
             JOIN indicators i ON i.id = ej.indicator_id
             ORDER BY
                CASE ej.status
                    WHEN 'pending' THEN 0
                    WHEN 'retrying' THEN 1
                    WHEN 'running' THEN 2
                    WHEN 'failed' THEN 3
                    ELSE 4
                END,
                ej.priority ASC,
                ej.next_attempt_at ASC,
                ej.created_at ASC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], Self::row_to_enrichment_job_with_context)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn row_to_enrichment_provider(
        row: &rusqlite::Row,
    ) -> rusqlite::Result<EnrichmentProviderRecord> {
        let supports_types_json: String = row.get(7)?;
        let created_at: String = row.get(8)?;
        let updated_at: String = row.get(9)?;
        Ok(EnrichmentProviderRecord {
            id: row.get(0)?,
            name: row.get(1)?,
            provider_type: row.get(2)?,
            enabled: row.get::<_, i64>(3)? != 0,
            config_json: row.get(4)?,
            secret_ref: row.get(5)?,
            rate_limit_per_minute: row.get(6)?,
            supports_types: indicator_types_from_json(&supports_types_json),
            created_at: parse_db_datetime(&created_at).unwrap_or_else(Utc::now),
            updated_at: parse_db_datetime(&updated_at).unwrap_or_else(Utc::now),
        })
    }

    fn row_to_enrichment_job(row: &rusqlite::Row) -> rusqlite::Result<EnrichmentJobRecord> {
        let next_attempt_at: String = row.get(6)?;
        let last_attempt_at: Option<String> = row.get(7)?;
        let created_at: String = row.get(9)?;
        let updated_at: String = row.get(10)?;
        Ok(EnrichmentJobRecord {
            id: row.get(0)?,
            indicator_id: row.get(1)?,
            provider_id: row.get(2)?,
            status: row.get(3)?,
            priority: row.get(4)?,
            attempt_count: row.get(5)?,
            next_attempt_at: parse_db_datetime(&next_attempt_at).unwrap_or_else(Utc::now),
            last_attempt_at: last_attempt_at.and_then(|value| parse_db_datetime(&value)),
            error_message: row.get(8)?,
            created_at: parse_db_datetime(&created_at).unwrap_or_else(Utc::now),
            updated_at: parse_db_datetime(&updated_at).unwrap_or_else(Utc::now),
        })
    }

    fn row_to_enrichment_result(row: &rusqlite::Row) -> rusqlite::Result<EnrichmentResultRecord> {
        let fetched_at: String = row.get(9)?;
        let expires_at: Option<String> = row.get(10)?;
        let created_at: String = row.get(11)?;
        let updated_at: String = row.get(12)?;
        Ok(EnrichmentResultRecord {
            id: row.get(0)?,
            indicator_id: row.get(1)?,
            provider_id: row.get(2)?,
            status: row.get(3)?,
            reputation: row.get(4)?,
            score: row.get(5)?,
            verdict: row.get(6)?,
            summary: row.get(7)?,
            raw_json: row.get(8)?,
            fetched_at: parse_db_datetime(&fetched_at).unwrap_or_else(Utc::now),
            expires_at: expires_at.and_then(|value| parse_db_datetime(&value)),
            created_at: parse_db_datetime(&created_at).unwrap_or_else(Utc::now),
            updated_at: parse_db_datetime(&updated_at).unwrap_or_else(Utc::now),
        })
    }

    fn row_to_enrichment_job_with_context(
        row: &rusqlite::Row,
    ) -> rusqlite::Result<EnrichmentJobWithContext> {
        let next_attempt_at: String = row.get(6)?;
        let last_attempt_at: Option<String> = row.get(7)?;
        let created_at: String = row.get(9)?;
        let updated_at: String = row.get(10)?;
        let indicator_type: String = row.get(13)?;
        Ok(EnrichmentJobWithContext {
            id: row.get(0)?,
            indicator_id: row.get(1)?,
            provider_id: row.get(2)?,
            status: row.get(3)?,
            priority: row.get(4)?,
            attempt_count: row.get(5)?,
            next_attempt_at: parse_db_datetime(&next_attempt_at).unwrap_or_else(Utc::now),
            last_attempt_at: last_attempt_at.and_then(|value| parse_db_datetime(&value)),
            error_message: row.get(8)?,
            created_at: parse_db_datetime(&created_at).unwrap_or_else(Utc::now),
            updated_at: parse_db_datetime(&updated_at).unwrap_or_else(Utc::now),
            provider_name: row.get(11)?,
            provider_type: row.get(12)?,
            indicator_type: indicator_type_from_db(&indicator_type),
            indicator_value: row.get(14)?,
        })
    }

    // ── Tags ────────────────────────────────────────────────────────────────

    pub fn create_tag(&self, tag: &TagCreate) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO tags (name, color, description) VALUES (?1, ?2, ?3)",
            params![tag.name, tag.color, tag.description],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_tag(&self, id: i64) -> Result<Option<Tag>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, color, description, created_at FROM tags WHERE id = ?1")?;
        stmt.query_row([id], Self::row_to_tag)
            .optional()
            .map_err(Into::into)
    }

    pub fn list_tags(&self) -> Result<Vec<Tag>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, color, description, created_at FROM tags ORDER BY name")?;
        let rows = stmt.query_map([], Self::row_to_tag)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn update_tag(&self, id: i64, tag: &TagUpdate) -> Result<()> {
        self.conn.execute(
            "UPDATE tags SET name = COALESCE(?1, name), color = COALESCE(?2, color), description = COALESCE(?3, description) WHERE id = ?4",
            params![tag.name.as_ref(), tag.color.as_ref(), tag.description.as_ref(), id],
        )?;
        Ok(())
    }

    pub fn delete_tag(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM tags WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn get_feed_tags(&self, feed_id: i64) -> Result<Vec<Tag>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.name, t.color, t.description, t.created_at
             FROM tags t JOIN feed_tags ft ON t.id = ft.tag_id WHERE ft.feed_id = ?1 ORDER BY t.name"
        )?;
        let rows = stmt.query_map([feed_id], Self::row_to_tag)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_keyword_tags(&self, keyword_id: i64) -> Result<Vec<Tag>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.name, t.color, t.description, t.created_at
             FROM tags t JOIN keyword_tags kt ON t.id = kt.tag_id WHERE kt.keyword_id = ?1 ORDER BY t.name"
        )?;
        let rows = stmt.query_map([keyword_id], Self::row_to_tag)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_alert_tags(&self, alert_id: i64) -> Result<Vec<Tag>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.name, t.color, t.description, t.created_at
             FROM tags t JOIN alert_tags at ON t.id = at.tag_id WHERE at.alert_id = ?1 ORDER BY t.name"
        )?;
        let rows = stmt.query_map([alert_id], Self::row_to_tag)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn assign_tag_to_feed(&self, feed_id: i64, tag_id: i64) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO feed_tags (feed_id, tag_id) VALUES (?1, ?2)",
            params![feed_id, tag_id],
        )?;
        Ok(())
    }

    pub fn remove_tag_from_feed(&self, feed_id: i64, tag_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM feed_tags WHERE feed_id = ?1 AND tag_id = ?2",
            params![feed_id, tag_id],
        )?;
        Ok(())
    }

    pub fn assign_tag_to_keyword(&self, keyword_id: i64, tag_id: i64) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO keyword_tags (keyword_id, tag_id) VALUES (?1, ?2)",
            params![keyword_id, tag_id],
        )?;
        Ok(())
    }

    pub fn remove_tag_from_keyword(&self, keyword_id: i64, tag_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM keyword_tags WHERE keyword_id = ?1 AND tag_id = ?2",
            params![keyword_id, tag_id],
        )?;
        Ok(())
    }

    pub fn assign_tag_to_alert(&self, alert_id: i64, tag_id: i64) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO alert_tags (alert_id, tag_id) VALUES (?1, ?2)",
            params![alert_id, tag_id],
        )?;
        Ok(())
    }

    pub fn remove_tag_from_alert(&self, alert_id: i64, tag_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM alert_tags WHERE alert_id = ?1 AND tag_id = ?2",
            params![alert_id, tag_id],
        )?;
        Ok(())
    }

    pub fn get_tag_usage_counts(&self) -> Result<HashMap<i64, i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT tag_id, SUM(cnt) FROM (
                SELECT tag_id, COUNT(*) AS cnt FROM feed_tags GROUP BY tag_id
                UNION ALL
                SELECT tag_id, COUNT(*) AS cnt FROM keyword_tags GROUP BY tag_id
                UNION ALL
                SELECT tag_id, COUNT(*) AS cnt FROM alert_tags GROUP BY tag_id
             ) GROUP BY tag_id",
        )?;
        let rows = stmt.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))?;
        rows.collect::<Result<HashMap<_, _>, _>>()
            .map_err(Into::into)
    }

    fn row_to_tag(row: &rusqlite::Row) -> rusqlite::Result<Tag> {
        let created: String = row.get(4)?;
        Ok(Tag {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
            description: row.get(3)?,
            created_at: parse_ts(&created).unwrap_or_else(Utc::now),
        })
    }

    // ── Notifications ─────────────────────────────────────────────────────

    pub fn create_notification(&self, cfg: &NotificationCreate) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO notifications (name, channel, config_json, enabled, min_criticality)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                cfg.name,
                format!("{:?}", cfg.channel),
                cfg.config_json,
                cfg.enabled as i64,
                format!("{:?}", cfg.min_criticality)
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_notifications(&self) -> Result<Vec<NotificationConfig>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, channel, config_json, enabled, min_criticality, created_at FROM notifications ORDER BY name"
        )?;
        let rows = stmt.query_map([], Self::row_to_notification)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn update_notification(&self, id: i64, cfg: &NotificationUpdate) -> Result<()> {
        self.conn.execute(
            "UPDATE notifications SET
                name = COALESCE(?1, name),
                channel = COALESCE(?2, channel),
                config_json = COALESCE(?3, config_json),
                enabled = COALESCE(?4, enabled),
                min_criticality = COALESCE(?5, min_criticality)
             WHERE id = ?6",
            params![
                cfg.name.as_ref(),
                cfg.channel.map(|c| format!("{:?}", c)),
                cfg.config_json.as_ref(),
                cfg.enabled.map(|v| v as i64),
                cfg.min_criticality.map(|c| format!("{:?}", c)),
                id
            ],
        )?;
        Ok(())
    }

    pub fn delete_notification(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM notifications WHERE id = ?1", [id])?;
        Ok(())
    }

    fn row_to_notification(row: &rusqlite::Row) -> rusqlite::Result<NotificationConfig> {
        let channel_str: String = row.get(2)?;
        let min_crit_str: String = row.get(5)?;
        let created: String = row.get(6)?;
        Ok(NotificationConfig {
            id: row.get(0)?,
            name: row.get(1)?,
            channel: NotificationChannel::from(channel_str.as_str()),
            config_json: row.get(3)?,
            enabled: row.get::<_, i64>(4)? != 0,
            min_criticality: Criticality::from(min_crit_str.as_str()),
            created_at: parse_ts(&created).unwrap_or_else(Utc::now),
        })
    }

    // ── Health Logs ───────────────────────────────────────────────────────

    pub fn add_health_log(
        &self,
        feed_id: i64,
        status: FeedStatus,
        error: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO feed_health_logs (feed_id, status, error_message) VALUES (?1, ?2, ?3)",
            params![feed_id, format!("{:?}", status), error],
        )?;
        Ok(())
    }

    pub fn get_health_logs(
        &self,
        feed_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<FeedHealthLog>> {
        let (sql, params) = if let Some(fid) = feed_id {
            (
                "SELECT id, feed_id, status, error_message, checked_at FROM feed_health_logs WHERE feed_id = ?1 ORDER BY checked_at DESC LIMIT ?2".to_string(),
                vec![Box::new(fid) as Box<dyn rusqlite::ToSql>, Box::new(limit as i64) as Box<dyn rusqlite::ToSql>]
            )
        } else {
            (
                "SELECT id, feed_id, status, error_message, checked_at FROM feed_health_logs ORDER BY checked_at DESC LIMIT ?1".to_string(),
                vec![Box::new(limit as i64) as Box<dyn rusqlite::ToSql>]
            )
        };
        let mut stmt = self.conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(rusqlite::params_from_iter(param_refs), |row| {
            Self::row_to_health_log(row)
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn prune_health_logs(&self, feed_id: i64, keep: usize) -> Result<()> {
        self.conn.execute(
            "DELETE FROM feed_health_logs WHERE id NOT IN (
                SELECT id FROM feed_health_logs WHERE feed_id = ?1 ORDER BY checked_at DESC LIMIT ?2
            ) AND feed_id = ?1",
            params![feed_id, keep as i64],
        )?;
        Ok(())
    }

    fn row_to_health_log(row: &rusqlite::Row) -> rusqlite::Result<FeedHealthLog> {
        let status_str: String = row.get(2)?;
        let checked_str: String = row.get(4)?;
        Ok(FeedHealthLog {
            id: row.get(0)?,
            feed_id: row.get(1)?,
            status: FeedStatus::from(status_str.as_str()),
            error_message: row.get(3)?,
            checked_at: parse_ts(&checked_str).unwrap_or_else(Utc::now),
        })
    }

    // ── Stats ───────────────────────────────────────────────────────────────

    pub fn get_stats(&self) -> Result<Stats> {
        let total_feeds: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM feeds", [], |row| row.get(0))?;
        let total_alerts: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM alerts", [], |row| row.get(0))?;
        let total_keywords: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM keywords", [], |row| row.get(0))?;
        let unread_alerts: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM alerts WHERE read = 0", [], |row| {
                    row.get(0)
                })?;
        let healthy_feeds: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM feeds WHERE enabled = 1 AND consecutive_failures = 0",
            [],
            |row| row.get(0),
        )?;
        let warning_feeds: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM feeds WHERE enabled = 1 AND consecutive_failures BETWEEN 1 AND 2",
            [],
            |row| row.get(0),
        )?;
        let error_feeds: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM feeds WHERE enabled = 1 AND consecutive_failures >= 3",
            [],
            |row| row.get(0),
        )?;
        let disabled_feeds: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM feeds WHERE enabled = 0", [], |row| {
                    row.get(0)
                })?;
        Ok(Stats {
            total_feeds,
            total_alerts,
            total_keywords,
            unread_alerts,
            healthy_feeds,
            warning_feeds,
            error_feeds,
            disabled_feeds,
        })
    }

    pub fn get_feed_health_ratio(&self) -> Result<f64> {
        let total: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM feeds WHERE enabled = 1", [], |row| {
                    row.get(0)
                })?;
        if total == 0 {
            return Ok(1.0);
        }
        let healthy: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM feeds WHERE enabled = 1 AND consecutive_failures = 0",
            [],
            |row| row.get(0),
        )?;
        Ok(healthy as f64 / total as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_db() -> Db {
        Db {
            conn: Connection::open_in_memory().unwrap(),
        }
    }

    fn create_diagnostic_test_feed(db: &Db) -> i64 {
        db.create_feed(&FeedCreate {
            name: "Example".into(),
            url: "https://example.test/feed.xml".into(),
            feed_type: FeedType::Rss,
            enabled: true,
            interval_secs: 300,
            api_template_id: None,
            api_key: None,
            custom_headers: None,
            tor_proxy: None,
        })
        .unwrap()
    }

    #[test]
    fn feed_summary_includes_fetch_diagnostic_fields() {
        let db = Db::new_in_memory_for_tests();
        db.init_schema().unwrap();
        let feed_id = db
            .create_feed(&FeedCreate {
                name: "Example".into(),
                url: "https://example.test/feed.xml".into(),
                feed_type: FeedType::Rss,
                enabled: true,
                interval_secs: 300,
                api_template_id: None,
                api_key: None,
                custom_headers: None,
                tor_proxy: None,
            })
            .unwrap();

        let feed = db.get_feed(feed_id).unwrap().unwrap();
        assert!(feed.last_fetch_success_at.is_none());
        assert!(feed.last_fetch_failed_at.is_none());
        assert!(feed.last_failure_phase.is_none());
        assert!(feed.last_failure_kind.is_none());
        assert!(feed.last_http_status.is_none());
    }

    #[test]
    fn records_fetch_attempts_newest_first() {
        let db = Db::new_in_memory_for_tests();
        db.init_schema().unwrap();
        let feed_id = create_diagnostic_test_feed(&db);

        let failed = crate::feed::diagnostics::FetchAttempt {
            id: None,
            feed_id: Some(feed_id),
            attempted_at: None,
            success: false,
            url: "https://example.test/feed.xml".into(),
            final_url: None,
            http_status: Some(404),
            elapsed_ms: 25,
            diagnostic: Some(crate::feed::diagnostics::classify_http_status(
                "https://example.test/feed.xml",
                404,
                25,
            )),
            items_seen: None,
            items_new: None,
        };

        db.record_feed_fetch_attempt(feed_id, &failed).unwrap();

        let attempts = db.list_feed_fetch_attempts(feed_id, 10).unwrap();
        assert_eq!(attempts.len(), 1);
        assert!(!attempts[0].success);
        assert_eq!(attempts[0].http_status, Some(404));
        assert_eq!(
            attempts[0].diagnostic.as_ref().unwrap().summary,
            "Feed returned HTTP 404"
        );
    }

    #[test]
    fn successful_attempt_resets_feed_failure_summary() {
        let db = Db::new_in_memory_for_tests();
        db.init_schema().unwrap();
        let feed_id = create_diagnostic_test_feed(&db);

        let attempt = crate::feed::diagnostics::FetchAttempt {
            id: None,
            feed_id: Some(feed_id),
            attempted_at: None,
            success: true,
            url: "https://example.test/feed.xml".into(),
            final_url: None,
            http_status: Some(200),
            elapsed_ms: 12,
            diagnostic: None,
            items_seen: Some(3),
            items_new: Some(2),
        };
        db.record_feed_fetch_outcome(feed_id, &attempt, Some("abc123"))
            .unwrap();

        let feed = db.get_feed(feed_id).unwrap().unwrap();
        assert_eq!(feed.consecutive_failures, 0);
        assert!(feed.last_error.is_none());
        assert!(feed.last_fetch_success_at.is_some());
        assert!(feed.last_fetch_failed_at.is_none());
        assert_eq!(feed.content_hash.as_deref(), Some("abc123"));
    }

    #[test]
    fn failed_attempt_updates_feed_failure_summary() {
        let db = Db::new_in_memory_for_tests();
        db.init_schema().unwrap();
        let feed_id = create_diagnostic_test_feed(&db);

        let attempt = crate::feed::diagnostics::FetchAttempt {
            id: None,
            feed_id: Some(feed_id),
            attempted_at: None,
            success: false,
            url: "https://example.test/feed.xml".into(),
            final_url: None,
            http_status: Some(404),
            elapsed_ms: 25,
            diagnostic: Some(crate::feed::diagnostics::classify_http_status(
                "https://example.test/feed.xml",
                404,
                25,
            )),
            items_seen: None,
            items_new: None,
        };
        db.record_feed_fetch_outcome(feed_id, &attempt, None)
            .unwrap();

        let feed = db.get_feed(feed_id).unwrap().unwrap();
        assert_eq!(feed.consecutive_failures, 1);
        assert_eq!(feed.last_error.as_deref(), Some("Feed returned HTTP 404"));
        assert_eq!(feed.last_failure_phase.as_deref(), Some("HTTP status"));
        assert_eq!(feed.last_failure_kind.as_deref(), Some("HTTP client error"));
        assert_eq!(feed.last_http_status, Some(404));
        assert!(feed.last_fetch_failed_at.is_some());
    }

    #[test]
    fn init_schema_seeds_demo_catalog_on_first_run() {
        let db = memory_db();
        db.init_schema().unwrap();

        assert_eq!(db.list_feeds(None).unwrap().len(), 83);
        assert_eq!(db.list_keywords(false).unwrap().len(), 11);
        assert_eq!(db.list_tags().unwrap().len(), 24);
        assert_eq!(db.list_templates().unwrap().len(), 2);
        assert_eq!(db.get_alert_count().unwrap(), 15);
        assert_eq!(db.get_health_logs(None, 50).unwrap().len(), 6);

        let feeds = db.list_feeds(None).unwrap();
        let krebs = feeds
            .iter()
            .find(|feed| feed.name == "Krebs On Security")
            .unwrap();
        assert_eq!(krebs.feed_type, FeedType::Rss);
        assert!(feeds
            .iter()
            .any(|feed| feed.url == "https://feeds.feedburner.com/TheHackersNews"));
        assert!(feeds
            .iter()
            .any(|feed| feed.url == "https://threatconnect.com/blog/feed/"));

        let distinct_feed_urls: std::collections::HashSet<&str> =
            feeds.iter().map(|feed| feed.url.as_str()).collect();
        assert_eq!(distinct_feed_urls.len(), feeds.len());

        let keyword_patterns: Vec<String> = db
            .list_keywords(false)
            .unwrap()
            .into_iter()
            .map(|keyword| keyword.pattern)
            .collect();
        assert!(keyword_patterns.contains(&"data breach".to_string()));
        assert!(keyword_patterns.contains(&"0day".to_string()));
        assert!(keyword_patterns.contains(&"leak".to_string()));

        let tag_names: Vec<String> = db
            .list_tags()
            .unwrap()
            .into_iter()
            .map(|tag| tag.name)
            .collect();
        assert!(tag_names.contains(&"Critical Infrastructure".to_string()));
        assert!(tag_names.contains(&"Financial".to_string()));
        assert!(tag_names.contains(&"Healthcare".to_string()));
        assert!(tag_names.contains(&"Native".to_string()));
        assert!(tag_names.contains(&"Global".to_string()));
    }

    #[test]
    fn init_schema_does_not_seed_catalog_when_database_already_has_feeds() {
        let db = memory_db();
        db.conn.execute_batch(include_str!("schema.sql")).unwrap();
        db.create_feed(&FeedCreate {
            name: "Existing feed".to_string(),
            url: "https://example.test/feed".to_string(),
            feed_type: FeedType::Rss,
            enabled: true,
            interval_secs: 300,
            ..FeedCreate::default()
        })
        .unwrap();

        db.init_schema().unwrap();

        let feeds = db.list_feeds(None).unwrap();
        assert!(feeds.iter().any(|feed| feed.name == "Existing feed"));
        assert!(feeds
            .iter()
            .any(|feed| feed.url == "https://feeds.feedburner.com/TheHackersNews"));
        assert_eq!(db.get_alert_count().unwrap(), 0);
    }

    #[test]
    fn feed_items_insert_idempotently_and_list_newest_first() {
        let db = memory_db();
        db.init_schema().unwrap();
        let feed_id = db
            .create_feed(&FeedCreate {
                name: "Example".into(),
                url: "https://example.com/feed.xml".into(),
                feed_type: FeedType::Rss,
                enabled: true,
                interval_secs: 300,
                ..FeedCreate::default()
            })
            .unwrap();

        let older = NewFeedItem {
            feed_id,
            title: "Older item".into(),
            url: Some("https://example.com/1".into()),
            author: None,
            summary: Some("<p>Hello&nbsp;world</p>".into()),
            content: None,
            published_at: Some(Utc::now() - Duration::days(1)),
            content_hash: "hash-older".into(),
            metadata_json: None,
        };
        let newer = NewFeedItem {
            feed_id,
            title: "Newer item".into(),
            url: Some("https://example.com/2".into()),
            author: Some("Analyst".into()),
            summary: None,
            content: Some("New content".into()),
            published_at: Some(Utc::now()),
            content_hash: "hash-newer".into(),
            metadata_json: Some(r#"{"id":2}"#.into()),
        };

        let first_id = db.upsert_feed_item(&older).unwrap();
        let duplicate_id = db.upsert_feed_item(&older).unwrap();
        db.upsert_feed_item(&newer).unwrap();

        let items = db.list_feed_items(&FeedItemFilter::default()).unwrap();
        assert_eq!(first_id, duplicate_id);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].item.title, "Newer item");
        assert_eq!(items[0].feed_name, "Example");
        assert_eq!(items[1].item.title, "Older item");

        let filtered = db
            .list_feed_items(&FeedItemFilter {
                text: Some("older".into()),
                ..FeedItemFilter::default()
            })
            .unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].item.content_hash, "hash-older");
    }

    #[test]
    fn feed_result_items_are_stored_without_keyword_matches() {
        let db = memory_db();
        db.init_schema().unwrap();
        let feed_id = db
            .create_feed(&FeedCreate {
                name: "Example".into(),
                url: "https://example.com/feed.xml".into(),
                feed_type: FeedType::Rss,
                enabled: true,
                interval_secs: 300,
                ..FeedCreate::default()
            })
            .unwrap();
        let feed = db.get_feed(feed_id).unwrap().unwrap();
        let result = FeedResult {
            content_hash: "feed-hash".into(),
            raw_content: "<rss />".into(),
            items: vec![FetchedFeedItem {
                title: Some("Stored article".into()),
                description: Some("Cached even without an alert".into()),
                date: Some(Utc::now()),
                url: Some("https://example.com/article".into()),
                source: Some("Reporter".into()),
                raw_json: None,
            }],
        };

        let inserted = db.store_feed_result_items(&feed, &result).unwrap();
        assert_eq!(inserted, 1);

        let items = db.list_feed_items(&FeedItemFilter::default()).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].item.title, "Stored article");
        assert_eq!(
            items[0].item.summary.as_deref(),
            Some("Cached even without an alert")
        );
        assert_eq!(items[0].item.author.as_deref(), Some("Reporter"));
    }

    #[test]
    fn feed_item_content_can_be_cached_after_article_fetch() {
        let db = memory_db();
        db.init_schema().unwrap();
        let feed_id = db
            .create_feed(&FeedCreate {
                name: "Example".into(),
                url: "https://example.com/feed.xml".into(),
                feed_type: FeedType::Rss,
                enabled: true,
                interval_secs: 300,
                ..FeedCreate::default()
            })
            .unwrap();
        let id = db
            .upsert_feed_item(&NewFeedItem {
                feed_id,
                title: "Article".into(),
                url: Some("https://example.com/article".into()),
                author: None,
                summary: Some("Short summary".into()),
                content: None,
                published_at: None,
                content_hash: "cache-content-hash".into(),
                metadata_json: None,
            })
            .unwrap();

        db.cache_feed_item_content(id, "Full extracted article body")
            .unwrap();

        let item = db.get_feed_item(id).unwrap().unwrap();
        assert_eq!(
            item.item.content.as_deref(),
            Some("Full extracted article body")
        );
    }

    #[test]
    fn indicators_are_upserted_linked_and_searchable() {
        let db = memory_db();
        db.init_schema().unwrap();
        let feed_id = db
            .create_feed(&FeedCreate {
                name: "IOC Feed".into(),
                url: "https://ioc.example.test/feed.xml".into(),
                feed_type: FeedType::Rss,
                enabled: true,
                interval_secs: 300,
                ..FeedCreate::default()
            })
            .unwrap();
        let keyword_id = db
            .create_keyword(&KeywordCreate {
                pattern: "ransomware".into(),
                is_regex: false,
                case_sensitive: false,
                criticality: Criticality::High,
                enabled: true,
            })
            .unwrap();
        let alert_id = db
            .create_alert(&AlertCreate {
                feed_id,
                keyword_id,
                title: Some("IOC alert".into()),
                content_snippet: "Observed CVE-2025-12345".into(),
                criticality: Criticality::High,
                content_hash: "ioc-alert-hash".into(),
                metadata_json: None,
            })
            .unwrap();

        let extracted = sentinel_ioc::ExtractedIndicator {
            indicator_type: IndicatorType::Cve,
            value: "cve-2025-12345".into(),
            normalized_value: "CVE-2025-12345".into(),
            source_field: "body".into(),
            start_offset: 9,
            end_offset: 23,
            surrounding_text: "Observed CVE-2025-12345".into(),
            confidence_hint: Some(90),
        };

        let first_id = db.upsert_indicator(&extracted).unwrap();
        let second_id = db.upsert_indicator(&extracted).unwrap();
        assert_eq!(first_id, second_id);

        db.store_extracted_indicators(&[extracted], Some(alert_id), None, Some(feed_id))
            .unwrap();

        let alert_indicators = db.list_indicators_for_alert(alert_id).unwrap();
        assert_eq!(alert_indicators.len(), 1);
        assert_eq!(alert_indicators[0].normalized_value, "CVE-2025-12345");
        assert_eq!(alert_indicators[0].sighting_count, 3);

        let detail = db
            .get_indicator_detail(alert_indicators[0].id)
            .unwrap()
            .unwrap();
        assert_eq!(detail.occurrences.len(), 1);
        assert_eq!(detail.occurrences[0].alert_id, Some(alert_id));
        assert_eq!(detail.occurrences[0].feed_id, Some(feed_id));

        let search_results = db
            .search_indicators(&IndicatorSearch {
                text: Some("2025-12345".into()),
                indicator_type: Some(IndicatorType::Cve),
                limit: Some(10),
            })
            .unwrap();
        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].id, first_id);
    }

    #[test]
    fn enrichment_providers_jobs_and_results_are_persisted() {
        let db = memory_db();
        db.init_schema().unwrap();
        let indicator = sentinel_ioc::ExtractedIndicator {
            indicator_type: IndicatorType::Cve,
            value: "CVE-2025-12345".into(),
            normalized_value: "CVE-2025-12345".into(),
            source_field: "body".into(),
            start_offset: 0,
            end_offset: 14,
            surrounding_text: "CVE-2025-12345".into(),
            confidence_hint: Some(90),
        };
        let indicator_id = db.upsert_indicator(&indicator).unwrap();
        let provider_id = db
            .create_enrichment_provider(&EnrichmentProviderCreate {
                name: "cisa-kev".into(),
                provider_type: "cisa_kev".into(),
                enabled: true,
                config_json: None,
                secret_ref: None,
                rate_limit_per_minute: Some(60),
                supports_types: vec![IndicatorType::Cve],
            })
            .unwrap();

        let providers = db.list_enabled_enrichment_providers().unwrap();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].name, "cisa-kev");
        assert_eq!(providers[0].supports_types, vec![IndicatorType::Cve]);

        let first_job = db
            .queue_enrichment_job(indicator_id, provider_id, 50)
            .unwrap();
        let duplicate_job = db
            .queue_enrichment_job(indicator_id, provider_id, 50)
            .unwrap();
        assert_eq!(first_job, duplicate_job);

        let claimed = db.claim_next_enrichment_jobs(5).unwrap();
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].id, first_job);
        assert_eq!(claimed[0].status, "running");

        db.store_enrichment_result(
            indicator_id,
            provider_id,
            &sentinel_enrichment::EnrichmentResult {
                provider_name: "cisa-kev".into(),
                indicator_type: IndicatorType::Cve,
                normalized_value: "CVE-2025-12345".into(),
                reputation: sentinel_enrichment::Reputation::Malicious,
                score: Some(95),
                verdict: Some("Known Exploited".into()),
                summary: Some("CISA KEV match".into()),
                raw_json: serde_json::json!({"known": true}),
                expires_at: None,
            },
        )
        .unwrap();
        db.mark_enrichment_job_succeeded(first_job).unwrap();

        let results = db.get_latest_enrichment_results(indicator_id).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].provider_id, provider_id);
        assert_eq!(results[0].reputation.as_deref(), Some("Malicious"));
        assert_eq!(results[0].score, Some(95));
        assert_eq!(results[0].verdict.as_deref(), Some("Known Exploited"));
        let indicator = db.get_indicator(indicator_id).unwrap().unwrap();
        assert_eq!(indicator.risk_score, Some(95));
    }

    #[test]
    fn enrichment_result_updates_indicator_risk_without_lowering_existing_score() {
        let db = memory_db();
        db.init_schema().unwrap();
        let indicator = sentinel_ioc::ExtractedIndicator {
            indicator_type: IndicatorType::Domain,
            value: "bad.example.net".into(),
            normalized_value: "bad.example.net".into(),
            source_field: "body".into(),
            start_offset: 0,
            end_offset: 15,
            surrounding_text: "bad.example.net".into(),
            confidence_hint: Some(70),
        };
        let indicator_id = db.upsert_indicator(&indicator).unwrap();
        let provider_id = db
            .create_enrichment_provider(&EnrichmentProviderCreate {
                name: "mock-risk".into(),
                provider_type: "mock".into(),
                enabled: true,
                supports_types: vec![IndicatorType::Domain],
                ..EnrichmentProviderCreate::default()
            })
            .unwrap();

        db.store_enrichment_result(
            indicator_id,
            provider_id,
            &sentinel_enrichment::EnrichmentResult {
                provider_name: "mock-risk".into(),
                indicator_type: IndicatorType::Domain,
                normalized_value: "bad.example.net".into(),
                reputation: sentinel_enrichment::Reputation::Suspicious,
                score: None,
                verdict: None,
                summary: None,
                raw_json: serde_json::json!({}),
                expires_at: None,
            },
        )
        .unwrap();
        assert_eq!(
            db.get_indicator(indicator_id).unwrap().unwrap().risk_score,
            Some(60)
        );

        db.store_enrichment_result(
            indicator_id,
            provider_id,
            &sentinel_enrichment::EnrichmentResult {
                provider_name: "mock-risk".into(),
                indicator_type: IndicatorType::Domain,
                normalized_value: "bad.example.net".into(),
                reputation: sentinel_enrichment::Reputation::Benign,
                score: None,
                verdict: None,
                summary: None,
                raw_json: serde_json::json!({}),
                expires_at: None,
            },
        )
        .unwrap();
        assert_eq!(
            db.get_indicator(indicator_id).unwrap().unwrap().risk_score,
            Some(60)
        );
    }

    #[test]
    fn enrichment_job_failure_increments_attempts_and_schedules_retry() {
        let db = memory_db();
        db.init_schema().unwrap();
        let indicator = sentinel_ioc::ExtractedIndicator {
            indicator_type: IndicatorType::Domain,
            value: "bad.example.net".into(),
            normalized_value: "bad.example.net".into(),
            source_field: "body".into(),
            start_offset: 0,
            end_offset: 15,
            surrounding_text: "bad.example.net".into(),
            confidence_hint: Some(65),
        };
        let indicator_id = db.upsert_indicator(&indicator).unwrap();
        let provider_id = db
            .create_enrichment_provider(&EnrichmentProviderCreate {
                name: "urlhaus".into(),
                provider_type: "urlhaus".into(),
                enabled: true,
                config_json: None,
                secret_ref: Some("threatdeck/urlhaus/api_key".into()),
                rate_limit_per_minute: Some(30),
                supports_types: vec![IndicatorType::Domain],
            })
            .unwrap();
        let job_id = db
            .queue_enrichment_job(indicator_id, provider_id, 100)
            .unwrap();
        let claimed = db.claim_next_enrichment_jobs(1).unwrap();
        assert_eq!(claimed.len(), 1);

        db.mark_enrichment_job_failed(job_id, "temporary failure", true)
            .unwrap();
        let retry_claim = db.claim_next_enrichment_jobs(1).unwrap();
        assert!(retry_claim.is_empty());

        let job = db.get_enrichment_job(job_id).unwrap().unwrap();
        assert_eq!(job.status, "retrying");
        assert_eq!(job.attempt_count, 1);
        assert_eq!(job.error_message.as_deref(), Some("temporary failure"));
    }

    #[test]
    fn enrichment_job_rate_limit_reschedules_without_incrementing_attempts() {
        let db = memory_db();
        db.init_schema().unwrap();
        let indicator = sentinel_ioc::ExtractedIndicator {
            indicator_type: IndicatorType::Domain,
            value: "bad.example.net".into(),
            normalized_value: "bad.example.net".into(),
            source_field: "body".into(),
            start_offset: 0,
            end_offset: 15,
            surrounding_text: "bad.example.net".into(),
            confidence_hint: Some(65),
        };
        let indicator_id = db.upsert_indicator(&indicator).unwrap();
        let provider_id = db
            .create_enrichment_provider(&EnrichmentProviderCreate {
                name: "urlhaus".into(),
                provider_type: "urlhaus".into(),
                enabled: true,
                supports_types: vec![IndicatorType::Domain],
                ..EnrichmentProviderCreate::default()
            })
            .unwrap();
        let job_id = db
            .queue_enrichment_job(indicator_id, provider_id, 100)
            .unwrap();

        db.mark_enrichment_job_rate_limited(job_id, "rate limit reached")
            .unwrap();

        let job = db.get_enrichment_job(job_id).unwrap().unwrap();
        assert_eq!(job.status, "rate_limited");
        assert_eq!(job.attempt_count, 0);
        assert_eq!(job.error_message.as_deref(), Some("rate limit reached"));
        assert!(db.claim_next_enrichment_jobs(1).unwrap().is_empty());
    }

    #[test]
    fn enrichment_queueing_uses_enabled_supported_providers_and_fresh_results() {
        let db = memory_db();
        db.init_schema().unwrap();
        let cve = sentinel_ioc::ExtractedIndicator {
            indicator_type: IndicatorType::Cve,
            value: "CVE-2025-12345".into(),
            normalized_value: "CVE-2025-12345".into(),
            source_field: "body".into(),
            start_offset: 0,
            end_offset: 14,
            surrounding_text: "CVE-2025-12345".into(),
            confidence_hint: Some(90),
        };
        let domain = sentinel_ioc::ExtractedIndicator {
            indicator_type: IndicatorType::Domain,
            value: "bad.example.net".into(),
            normalized_value: "bad.example.net".into(),
            source_field: "body".into(),
            start_offset: 15,
            end_offset: 30,
            surrounding_text: "bad.example.net".into(),
            confidence_hint: Some(65),
        };
        let ids = db
            .store_extracted_indicators(&[cve, domain], None, None, None)
            .unwrap();
        let cisa_id = db
            .create_enrichment_provider(&EnrichmentProviderCreate {
                name: "cisa-kev".into(),
                provider_type: "cisa_kev".into(),
                enabled: true,
                supports_types: vec![IndicatorType::Cve],
                ..EnrichmentProviderCreate::default()
            })
            .unwrap();
        db.create_enrichment_provider(&EnrichmentProviderCreate {
            name: "disabled-urlhaus".into(),
            provider_type: "urlhaus".into(),
            enabled: false,
            supports_types: vec![IndicatorType::Domain],
            ..EnrichmentProviderCreate::default()
        })
        .unwrap();
        db.create_enrichment_provider(&EnrichmentProviderCreate {
            name: "enabled-urlhaus".into(),
            provider_type: "urlhaus".into(),
            enabled: true,
            supports_types: vec![IndicatorType::Domain],
            ..EnrichmentProviderCreate::default()
        })
        .unwrap();

        let queued = db.queue_enrichment_jobs_for_indicators(&ids).unwrap();
        assert_eq!(queued.len(), 2);

        let duplicate = db.queue_enrichment_jobs_for_indicators(&ids).unwrap();
        assert_eq!(duplicate.len(), 2);
        assert_eq!(queued, duplicate);

        db.store_enrichment_result(
            ids[0],
            cisa_id,
            &sentinel_enrichment::EnrichmentResult {
                provider_name: "cisa-kev".into(),
                indicator_type: IndicatorType::Cve,
                normalized_value: "CVE-2025-12345".into(),
                reputation: sentinel_enrichment::Reputation::Malicious,
                score: Some(95),
                verdict: Some("Known Exploited".into()),
                summary: Some("Fresh KEV result".into()),
                raw_json: serde_json::json!({"fresh": true}),
                expires_at: Some(Utc::now() + Duration::hours(24)),
            },
        )
        .unwrap();

        db.mark_enrichment_job_succeeded(queued[0]).unwrap();
        let after_fresh_result = db.queue_enrichment_jobs_for_indicators(&[ids[0]]).unwrap();
        assert!(after_fresh_result.is_empty());
    }

    #[test]
    fn enrichment_jobs_can_be_listed_for_troubleshooting() {
        let db = memory_db();
        db.init_schema().unwrap();
        let indicator = sentinel_ioc::ExtractedIndicator {
            indicator_type: IndicatorType::Cve,
            value: "CVE-2025-12345".into(),
            normalized_value: "CVE-2025-12345".into(),
            source_field: "body".into(),
            start_offset: 0,
            end_offset: 14,
            surrounding_text: "CVE-2025-12345".into(),
            confidence_hint: Some(90),
        };
        let indicator_id = db.upsert_indicator(&indicator).unwrap();
        let provider = db
            .list_enabled_enrichment_providers()
            .unwrap()
            .into_iter()
            .find(|provider| provider.name == "cisa-kev")
            .unwrap();
        let job_id = db
            .queue_enrichment_job(indicator_id, provider.id, 25)
            .unwrap();

        let jobs = db.list_enrichment_jobs(10).unwrap();

        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, job_id);
        assert_eq!(jobs[0].provider_name, "cisa-kev");
        assert_eq!(jobs[0].indicator_value, "CVE-2025-12345");
        assert_eq!(jobs[0].indicator_type, IndicatorType::Cve);
        assert_eq!(jobs[0].status, "pending");
        assert_eq!(jobs[0].priority, 25);
    }

    #[test]
    fn enrichment_providers_can_be_listed_and_toggled_by_name() {
        let db = memory_db();
        db.init_schema().unwrap();

        db.set_enrichment_provider_enabled("cisa-kev", false)
            .unwrap();
        assert!(db.list_enabled_enrichment_providers().unwrap().is_empty());

        let providers = db.list_enrichment_providers().unwrap();
        let cisa = providers
            .iter()
            .find(|provider| provider.name == "cisa-kev")
            .expect("provider still listed");
        assert!(!cisa.enabled);

        db.set_enrichment_provider_enabled("cisa-kev", true)
            .unwrap();
        assert_eq!(db.list_enabled_enrichment_providers().unwrap().len(), 1);
    }

    #[test]
    fn init_schema_does_not_reseed_after_seed_marker_exists() {
        let db = memory_db();
        db.init_schema().unwrap();
        db.conn.execute("DELETE FROM alerts", []).unwrap();
        db.conn.execute("DELETE FROM feeds", []).unwrap();
        db.conn.execute("DELETE FROM keywords", []).unwrap();

        db.init_schema().unwrap();

        assert_eq!(db.list_feeds(None).unwrap().len(), 0);
        assert_eq!(db.list_keywords(false).unwrap().len(), 0);
        assert_eq!(db.get_alert_count().unwrap(), 0);
    }

    #[test]
    fn init_schema_seeds_builtin_enrichment_providers() {
        let db = memory_db();
        db.init_schema().unwrap();

        let providers = db.list_enrichment_providers().unwrap();
        let cisa = providers
            .iter()
            .find(|provider| provider.name == "cisa-kev")
            .expect("CISA KEV provider seeded");
        assert_eq!(cisa.provider_type, "cisa_kev");
        assert_eq!(cisa.supports_types, vec![IndicatorType::Cve]);
        assert!(cisa.enabled);

        let urlhaus = providers
            .iter()
            .find(|provider| provider.name == "urlhaus")
            .expect("URLHaus provider seeded");
        assert_eq!(urlhaus.provider_type, "urlhaus");
        assert_eq!(
            urlhaus.supports_types,
            vec![
                IndicatorType::Url,
                IndicatorType::Domain,
                IndicatorType::Ipv4,
                IndicatorType::Md5,
                IndicatorType::Sha256,
            ]
        );
        assert!(!urlhaus.enabled);
        assert_eq!(urlhaus.secret_ref.as_deref(), Some("env:URLHAUS_AUTH_KEY"));
    }

    #[test]
    fn triage_note_is_persisted_and_retrievable() {
        let db = memory_db();
        db.init_schema().unwrap();

        let feed_id = db
            .create_feed(&FeedCreate {
                name: "Test Feed".into(),
                url: "https://example.com/feed.xml".into(),
                feed_type: FeedType::Rss,
                enabled: true,
                interval_secs: 300,
                ..FeedCreate::default()
            })
            .unwrap();

        let keyword_id = db
            .create_keyword(&KeywordCreate {
                pattern: "breach".into(),
                criticality: Criticality::High,
                enabled: true,
                ..KeywordCreate::default()
            })
            .unwrap();

        let alert_id = db
            .create_alert(&AlertCreate {
                feed_id,
                keyword_id,
                title: Some("Triage note test".into()),
                content_snippet: "Test content".into(),
                criticality: Criticality::High,
                content_hash: "triage-note-hash".into(),
                metadata_json: None,
            })
            .unwrap();

        // Verify defaults
        let alert = db.get_alert(alert_id).unwrap().unwrap();
        assert_eq!(alert.status, AlertStatus::New);
        assert_eq!(alert.disposition, AlertDisposition::Unknown);
        assert!(alert.triage_notes.is_none());

        // Add a note
        db.add_alert_note(alert_id, "First investigation note")
            .unwrap();

        // Verify via get_alert
        let alert = db.get_alert(alert_id).unwrap().unwrap();
        assert_eq!(
            alert.triage_notes.as_deref(),
            Some("First investigation note")
        );

        // Add second note
        db.add_alert_note(alert_id, "Second follow-up note")
            .unwrap();

        let alert = db.get_alert(alert_id).unwrap().unwrap();
        assert!(alert
            .triage_notes
            .as_deref()
            .unwrap()
            .contains("First investigation note"));
        assert!(alert
            .triage_notes
            .as_deref()
            .unwrap()
            .contains("Second follow-up note"));

        // Verify via list_alerts
        let alerts = db.list_alerts(&AlertFilter::default()).unwrap();
        let found = alerts.iter().find(|a| a.alert.id == alert_id).unwrap();
        assert_eq!(
            found.alert.triage_notes.as_deref(),
            Some(alert.triage_notes.as_deref().unwrap())
        );

        // Verify history
        let events = db.list_alert_triage_events(alert_id).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "note_added");
        assert_eq!(events[0].note.as_deref(), Some("First investigation note"));
        assert_eq!(events[1].event_type, "note_added");
        assert_eq!(events[1].note.as_deref(), Some("Second follow-up note"));
    }

    #[test]
    fn triage_status_transitions_persist_and_log_events() {
        let db = memory_db();
        db.init_schema().unwrap();

        let feed_id = db
            .create_feed(&FeedCreate {
                name: "Test Feed".into(),
                url: "https://example.com/feed.xml".into(),
                feed_type: FeedType::Rss,
                enabled: true,
                interval_secs: 300,
                ..FeedCreate::default()
            })
            .unwrap();

        let keyword_id = db
            .create_keyword(&KeywordCreate {
                pattern: "breach".into(),
                criticality: Criticality::High,
                enabled: true,
                ..KeywordCreate::default()
            })
            .unwrap();

        let alert_id = db
            .create_alert(&AlertCreate {
                feed_id,
                keyword_id,
                title: Some("Status transition test".into()),
                content_snippet: "Test content".into(),
                criticality: Criticality::High,
                content_hash: "status-transition-hash".into(),
                metadata_json: None,
            })
            .unwrap();

        // Acknowledge
        db.update_alert_status(alert_id, AlertStatus::Acknowledged, None)
            .unwrap();
        let alert = db.get_alert(alert_id).unwrap().unwrap();
        assert_eq!(alert.status, AlertStatus::Acknowledged);
        assert!(alert.acknowledged_at.is_some());

        // Investigate
        db.update_alert_status(alert_id, AlertStatus::Investigating, None)
            .unwrap();
        let alert = db.get_alert(alert_id).unwrap().unwrap();
        assert_eq!(alert.status, AlertStatus::Investigating);
        assert!(alert.investigating_at.is_some());

        // Close
        db.update_alert_disposition(alert_id, AlertDisposition::FalsePositive, None)
            .unwrap();
        db.close_alert(
            alert_id,
            AlertDisposition::FalsePositive,
            Some("Verified benign"),
        )
        .unwrap();
        let alert = db.get_alert(alert_id).unwrap().unwrap();
        assert_eq!(alert.status, AlertStatus::Closed);
        assert_eq!(alert.disposition, AlertDisposition::FalsePositive);
        assert!(alert.closed_at.is_some());
        assert_eq!(alert.closed_reason.as_deref(), Some("Verified benign"));

        // Reopen
        db.reopen_alert(alert_id, Some("Re-opening for review"))
            .unwrap();
        let alert = db.get_alert(alert_id).unwrap().unwrap();
        assert_eq!(alert.status, AlertStatus::Acknowledged);
        assert!(alert.closed_at.is_none());

        // Verify events
        let events = db.list_alert_triage_events(alert_id).unwrap();
        let event_types: Vec<_> = events.iter().map(|e| e.event_type.as_str()).collect();
        assert!(event_types.contains(&"status_changed"));
        assert!(event_types.contains(&"disposition_changed"));
        assert!(event_types.contains(&"closed"));
        assert!(event_types.contains(&"reopened"));
    }

    #[test]
    fn list_alerts_filters_use_bound_parameters() {
        let db = memory_db();
        db.init_schema().unwrap();
        db.conn.execute("DELETE FROM alerts", []).unwrap();
        db.conn.execute("DELETE FROM alert_tags", []).unwrap();

        let feed_id = db
            .create_feed(&FeedCreate {
                name: "Filter Feed".into(),
                url: "https://filter.test/feed.xml".into(),
                feed_type: FeedType::Rss,
                enabled: true,
                interval_secs: 300,
                ..FeedCreate::default()
            })
            .unwrap();
        let other_feed_id = db
            .create_feed(&FeedCreate {
                name: "Other Feed".into(),
                url: "https://other.test/feed.xml".into(),
                feed_type: FeedType::Rss,
                enabled: true,
                interval_secs: 300,
                ..FeedCreate::default()
            })
            .unwrap();

        let keyword_id = db
            .create_keyword(&KeywordCreate {
                pattern: "breach".into(),
                criticality: Criticality::High,
                enabled: true,
                ..KeywordCreate::default()
            })
            .unwrap();
        let other_keyword_id = db
            .create_keyword(&KeywordCreate {
                pattern: "phishing".into(),
                criticality: Criticality::Medium,
                enabled: true,
                ..KeywordCreate::default()
            })
            .unwrap();

        let tag_id = db
            .create_tag(&TagCreate {
                name: "critical".into(),
                color: "#ff0000".into(),
                description: None,
            })
            .unwrap();

        let high_id = db
            .create_alert(&AlertCreate {
                feed_id,
                keyword_id,
                title: Some("High breach alert".into()),
                content_snippet: "Sensitive data breach content".into(),
                criticality: Criticality::High,
                content_hash: "high-hash".into(),
                metadata_json: None,
            })
            .unwrap();
        db.assign_tag_to_alert(high_id, tag_id).unwrap();
        db.assign_alert_owner(high_id, Some("alice"), Some("assigned to alice"))
            .unwrap();
        db.update_alert_status(high_id, AlertStatus::Acknowledged, None)
            .unwrap();
        db.update_alert_disposition(high_id, AlertDisposition::ConfirmedThreat, None)
            .unwrap();

        let medium_id = db
            .create_alert(&AlertCreate {
                feed_id: other_feed_id,
                keyword_id: other_keyword_id,
                title: Some("Medium phishing alert".into()),
                content_snippet: "Phishing email content".into(),
                criticality: Criticality::Medium,
                content_hash: "medium-hash".into(),
                metadata_json: None,
            })
            .unwrap();
        db.update_alert_status(medium_id, AlertStatus::Closed, Some("Verified benign"))
            .unwrap();
        db.update_alert_disposition(medium_id, AlertDisposition::Benign, None)
            .unwrap();

        // Default returns both, most-recent first.
        let all = db.list_alerts(&AlertFilter::default()).unwrap();
        assert_eq!(all.len(), 2);

        // criticality
        let high = db
            .list_alerts(&AlertFilter {
                criticality: Some(Criticality::High),
                ..AlertFilter::default()
            })
            .unwrap();
        assert_eq!(high.len(), 1);
        assert_eq!(high[0].alert.id, high_id);

        // feed_id
        let by_feed = db
            .list_alerts(&AlertFilter {
                feed_id: Some(other_feed_id),
                ..AlertFilter::default()
            })
            .unwrap();
        assert_eq!(by_feed.len(), 1);
        assert_eq!(by_feed[0].alert.id, medium_id);

        // keyword_id
        let by_keyword = db
            .list_alerts(&AlertFilter {
                keyword_id: Some(keyword_id),
                ..AlertFilter::default()
            })
            .unwrap();
        assert_eq!(by_keyword.len(), 1);
        assert_eq!(by_keyword[0].alert.id, high_id);

        // tag_id
        let by_tag = db
            .list_alerts(&AlertFilter {
                tag_id: Some(tag_id),
                ..AlertFilter::default()
            })
            .unwrap();
        assert_eq!(by_tag.len(), 1);
        assert_eq!(by_tag[0].alert.id, high_id);

        // status
        let by_status = db
            .list_alerts(&AlertFilter {
                status: Some(AlertStatus::Closed),
                ..AlertFilter::default()
            })
            .unwrap();
        assert_eq!(by_status.len(), 1);
        assert_eq!(by_status[0].alert.id, medium_id);

        // disposition
        let by_disp = db
            .list_alerts(&AlertFilter {
                disposition: Some(AlertDisposition::ConfirmedThreat),
                ..AlertFilter::default()
            })
            .unwrap();
        assert_eq!(by_disp.len(), 1);
        assert_eq!(by_disp[0].alert.id, high_id);

        // owner
        let by_owner = db
            .list_alerts(&AlertFilter {
                owner: Some("alice".into()),
                ..AlertFilter::default()
            })
            .unwrap();
        assert_eq!(by_owner.len(), 1);
        assert_eq!(by_owner[0].alert.id, high_id);

        // text search across multiple columns
        let by_text = db
            .list_alerts(&AlertFilter {
                text: Some("phishing".into()),
                ..AlertFilter::default()
            })
            .unwrap();
        assert_eq!(by_text.len(), 1);
        assert_eq!(by_text[0].alert.id, medium_id);

        // combined filter: text + status + limit
        let combined = db
            .list_alerts(&AlertFilter {
                text: Some("phishing".into()),
                status: Some(AlertStatus::Closed),
                limit: Some(5),
                ..AlertFilter::default()
            })
            .unwrap();
        assert_eq!(combined.len(), 1);
        assert_eq!(combined[0].alert.id, medium_id);

        // open_only / closed_only
        let open_only = db
            .list_alerts(&AlertFilter {
                open_only: true,
                ..AlertFilter::default()
            })
            .unwrap();
        assert_eq!(open_only.len(), 1);
        assert_eq!(open_only[0].alert.id, high_id);

        let closed_only = db
            .list_alerts(&AlertFilter {
                closed_only: true,
                ..AlertFilter::default()
            })
            .unwrap();
        assert_eq!(closed_only.len(), 1);
        assert_eq!(closed_only[0].alert.id, medium_id);

        // limit is respected (combined with text to exercise parameter numbering)
        let limited = db
            .list_alerts(&AlertFilter {
                text: Some("alert".into()),
                limit: Some(1),
                ..AlertFilter::default()
            })
            .unwrap();
        assert_eq!(limited.len(), 1);
    }
}

// ── FeedStatus FromStr ─────────────────────────────────────────────────────

impl From<&str> for FeedStatus {
    fn from(s: &str) -> Self {
        match s {
            "Healthy" => FeedStatus::Healthy,
            "Warning" => FeedStatus::Warning,
            "Error" => FeedStatus::Error,
            "Disabled" => FeedStatus::Disabled,
            _ => FeedStatus::Healthy,
        }
    }
}

// ── Create/Update Structs ──────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct FeedCreate {
    pub name: String,
    pub url: String,
    pub feed_type: FeedType,
    pub enabled: bool,
    pub interval_secs: u64,
    pub api_template_id: Option<i64>,
    pub api_key: Option<String>,
    pub custom_headers: Option<String>,
    pub tor_proxy: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct FeedUpdate {
    pub name: Option<String>,
    pub url: Option<String>,
    pub feed_type: Option<FeedType>,
    pub enabled: Option<bool>,
    pub interval_secs: Option<u64>,
    pub api_template_id: Option<i64>,
    pub api_key: Option<String>,
    pub custom_headers: Option<String>,
    pub tor_proxy: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ApiTemplateCreate {
    pub name: String,
    pub jsonpath_title: String,
    pub jsonpath_description: String,
    pub jsonpath_date: String,
    pub jsonpath_url: String,
    pub jsonpath_source: String,
    pub pagination_config: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct KeywordCreate {
    pub pattern: String,
    pub is_regex: bool,
    pub case_sensitive: bool,
    pub criticality: Criticality,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default)]
pub struct KeywordUpdate {
    pub pattern: Option<String>,
    pub is_regex: Option<bool>,
    pub case_sensitive: Option<bool>,
    pub criticality: Option<Criticality>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct AlertCreate {
    pub feed_id: i64,
    pub keyword_id: i64,
    pub title: Option<String>,
    pub content_snippet: String,
    pub criticality: Criticality,
    pub content_hash: String,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct AlertFilter {
    pub text: Option<String>,
    pub criticality: Option<Criticality>,
    pub unread_only: bool,
    pub tag_id: Option<i64>,
    pub feed_id: Option<i64>,
    pub keyword_id: Option<i64>,
    pub limit: Option<i64>,
    pub status: Option<AlertStatus>,
    pub disposition: Option<AlertDisposition>,
    pub owner: Option<String>,
    pub open_only: bool,
    pub closed_only: bool,
}

#[derive(Debug, Clone, Default)]
pub struct IndicatorSearch {
    pub text: Option<String>,
    pub indicator_type: Option<IndicatorType>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct IndicatorRecord {
    pub id: i64,
    pub indicator_type: IndicatorType,
    pub value: String,
    pub normalized_value: String,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub sighting_count: i64,
    pub confidence_score: Option<i64>,
    pub risk_score: Option<i64>,
    pub metadata_json: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct IndicatorOccurrenceCreate {
    pub indicator_id: i64,
    pub content_item_id: Option<i64>,
    pub alert_id: Option<i64>,
    pub feed_id: Option<i64>,
    pub source_field: Option<String>,
    pub start_offset: Option<i64>,
    pub end_offset: Option<i64>,
    pub surrounding_text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IndicatorOccurrence {
    pub id: i64,
    pub indicator_id: i64,
    pub content_item_id: Option<i64>,
    pub alert_id: Option<i64>,
    pub feed_id: Option<i64>,
    pub source_field: Option<String>,
    pub start_offset: Option<i64>,
    pub end_offset: Option<i64>,
    pub surrounding_text: Option<String>,
    pub detected_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct IndicatorDetail {
    pub indicator: IndicatorRecord,
    pub occurrences: Vec<IndicatorOccurrence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredFeedItem {
    pub id: i64,
    pub inserted: bool,
}

#[derive(Debug, Clone, Default)]
pub struct EnrichmentProviderCreate {
    pub name: String,
    pub provider_type: String,
    pub enabled: bool,
    pub config_json: Option<String>,
    pub secret_ref: Option<String>,
    pub rate_limit_per_minute: Option<u32>,
    pub supports_types: Vec<IndicatorType>,
}

#[derive(Debug, Clone, Default)]
pub struct EnrichmentProviderUpdate {
    pub id: i64,
    pub enabled: Option<bool>,
    pub config_json: Option<String>,
    pub secret_ref: Option<String>,
    pub rate_limit_per_minute: Option<u32>,
    pub supports_types: Option<Vec<IndicatorType>>,
}

#[derive(Debug, Clone)]
pub struct EnrichmentProviderRecord {
    pub id: i64,
    pub name: String,
    pub provider_type: String,
    pub enabled: bool,
    pub config_json: Option<String>,
    pub secret_ref: Option<String>,
    pub rate_limit_per_minute: Option<i64>,
    pub supports_types: Vec<IndicatorType>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct EnrichmentJobRecord {
    pub id: i64,
    pub indicator_id: i64,
    pub provider_id: i64,
    pub status: String,
    pub priority: i64,
    pub attempt_count: i64,
    pub next_attempt_at: DateTime<Utc>,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct EnrichmentJobWithContext {
    pub id: i64,
    pub indicator_id: i64,
    pub provider_id: i64,
    pub status: String,
    pub priority: i64,
    pub attempt_count: i64,
    pub next_attempt_at: DateTime<Utc>,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub provider_name: String,
    pub provider_type: String,
    pub indicator_type: IndicatorType,
    pub indicator_value: String,
}

#[derive(Debug, Clone)]
pub struct EnrichmentResultRecord {
    pub id: i64,
    pub indicator_id: i64,
    pub provider_id: i64,
    pub status: String,
    pub reputation: Option<String>,
    pub score: Option<i64>,
    pub verdict: Option<String>,
    pub summary: Option<String>,
    pub raw_json: Option<String>,
    pub fetched_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct TagCreate {
    pub name: String,
    pub color: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct TagUpdate {
    pub name: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct NotificationCreate {
    pub name: String,
    pub channel: NotificationChannel,
    pub config_json: String,
    pub enabled: bool,
    pub min_criticality: Criticality,
}

#[derive(Debug, Clone, Default)]
pub struct NotificationUpdate {
    pub name: Option<String>,
    pub channel: Option<NotificationChannel>,
    pub config_json: Option<String>,
    pub enabled: Option<bool>,
    pub min_criticality: Option<Criticality>,
}

#[derive(Debug, Clone)]
pub struct AlertReportData {
    pub alert: Alert,
    pub feed_name: String,
    pub feed_type: String,
    pub feed_url: String,
    pub keyword_pattern: String,
    pub keyword_match_type: String,
    pub keyword_criticality: String,
    pub tags: Vec<Tag>,
    pub indicators: Vec<IndicatorRecord>,
    pub triage_history: Vec<AlertTriageEvent>,
}

#[derive(Debug, Clone)]
pub struct AlertTriageEvent {
    pub id: i64,
    pub alert_id: i64,
    pub event_type: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub actor: Option<String>,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct FeedHealthRecord {
    pub id: i64,
    pub name: String,
    pub feed_type: String,
    pub enabled: bool,
    pub status: String,
    pub consecutive_failures: i64,
    pub last_fetch_at: Option<DateTime<Utc>>,
    pub last_fetch_success_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

impl Db {
    pub fn get_alert_report_data(&self, alert_id: i64) -> Result<Option<AlertReportData>> {
        let alert = match self.get_alert(alert_id)? {
            Some(a) => a,
            None => return Ok(None),
        };

        let feed = self.get_feed(alert.feed_id)?;
        let keyword = self.get_keyword(alert.keyword_id)?;

        let tags = self.get_alert_tags(alert_id)?;
        let indicators = self.list_indicators_for_alert(alert_id)?;
        let triage_history = self.list_alert_triage_events(alert_id)?;

        let feed_name = feed.as_ref().map(|f| f.name.clone()).unwrap_or_default();
        let feed_type = feed
            .as_ref()
            .map(|f| format!("{:?}", f.feed_type))
            .unwrap_or_default();
        let feed_url = feed.as_ref().map(|f| f.url.clone()).unwrap_or_default();

        let keyword_pattern = keyword
            .as_ref()
            .map(|k| k.pattern.clone())
            .unwrap_or_default();
        let keyword_match_type = keyword
            .as_ref()
            .map(|k| {
                if k.is_regex {
                    "Regex".to_string()
                } else {
                    "Simple".to_string()
                }
            })
            .unwrap_or_default();
        let keyword_criticality = keyword
            .as_ref()
            .map(|k| format!("{:?}", k.criticality))
            .unwrap_or_default();

        Ok(Some(AlertReportData {
            alert,
            feed_name,
            feed_type,
            feed_url,
            keyword_pattern,
            keyword_match_type,
            keyword_criticality,
            tags,
            indicators,
            triage_history,
        }))
    }

    pub fn list_feed_health(&self) -> Result<Vec<FeedHealthRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, feed_type, enabled, consecutive_failures, last_fetch_at,
                    last_fetch_success_at, last_error
             FROM feeds
             ORDER BY name",
        )?;

        let rows = stmt.query_map([], |row| {
            let enabled: i64 = row.get(3)?;
            let consecutive_failures: i64 = row.get(4)?;

            let status = if enabled == 0 {
                "Disabled"
            } else if consecutive_failures >= 5 {
                "Error"
            } else if consecutive_failures >= 2 {
                "Warning"
            } else {
                "Healthy"
            };

            let feed_type_str: String = row.get(2)?;

            Ok(FeedHealthRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                feed_type: feed_type_str,
                enabled: enabled != 0,
                status: status.to_string(),
                consecutive_failures,
                last_fetch_at: row.get::<_, Option<String>>(5)?.and_then(|s| parse_ts(&s)),
                last_fetch_success_at: row.get::<_, Option<String>>(6)?.and_then(|s| parse_ts(&s)),
                last_error: row.get(7)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

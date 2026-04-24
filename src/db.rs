use anyhow::{Context, Result};
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

use crate::types::*;

pub struct Db {
    conn: Connection,
}

/// Parse SQLite timestamp string (format: "YYYY-MM-DD HH:MM:SS") to UTC DateTime.
fn parse_ts(s: &str) -> Option<DateTime<Utc>> {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok()
        .map(|dt| DateTime::from_naive_utc_and_offset(dt, Utc))
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("opening database at {}", path.display()))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        Ok(Self { conn })
    }

    pub fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(include_str!("schema.sql"))?;
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
                    consecutive_failures, content_hash, created_at, api_template_id, api_key, custom_headers, tor_proxy
             FROM feeds WHERE id = ?1"
        )?;
        stmt.query_row([id], Self::row_to_feed).optional().map_err(Into::into)
    }

    pub fn list_feeds(&self, filter: Option<&str>) -> Result<Vec<Feed>> {
        let has_filter = filter.map(|f| !f.is_empty()).unwrap_or(false);
        let sql = if has_filter {
            "SELECT id, name, url, feed_type, enabled, interval_secs, last_fetch_at, last_error,
                    consecutive_failures, content_hash, created_at, api_template_id, api_key, custom_headers, tor_proxy
             FROM feeds WHERE name LIKE ?1 OR url LIKE ?1 ORDER BY id"
        } else {
            "SELECT id, name, url, feed_type, enabled, interval_secs, last_fetch_at, last_error,
                    consecutive_failures, content_hash, created_at, api_template_id, api_key, custom_headers, tor_proxy
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
                feed.name.as_ref(), feed.url.as_ref(),
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

    pub fn update_feed_health(&self, id: i64, success: bool, error: Option<&str>, content_hash: Option<&str>) -> Result<()> {
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
        self.conn.execute(
            "UPDATE feeds SET enabled = NOT enabled WHERE id = ?1",
            [id],
        )?;
        Ok(())
    }

    fn row_to_feed(row: &rusqlite::Row) -> rusqlite::Result<Feed> {
        let feed_type_str: String = row.get(3)?;
        let last_fetch: Option<String> = row.get(6)?;
        let created: String = row.get(10)?;
        Ok(Feed {
            id: row.get(0)?,
            name: row.get(1)?,
            url: row.get(2)?,
            feed_type: FeedType::from(feed_type_str.as_str()),
            enabled: row.get::<_, i64>(4)? != 0,
            interval_secs: row.get::<_, i64>(5)? as u64,
            last_fetch_at: last_fetch.and_then(|s| parse_ts(&s)),
            last_error: row.get(7)?,
            consecutive_failures: row.get::<_, i64>(8)? as u32,
            content_hash: row.get(9)?,
            created_at: parse_ts(&created).unwrap_or_else(|| Utc::now()),
            api_template_id: row.get(11)?,
            api_key: row.get(12)?,
            custom_headers: row.get(13)?,
            tor_proxy: row.get(14)?,
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
        stmt.query_row([id], Self::row_to_template).optional().map_err(Into::into)
    }

    pub fn get_template_by_name(&self, name: &str) -> Result<Option<ApiTemplate>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, jsonpath_title, jsonpath_description, jsonpath_date, jsonpath_url, jsonpath_source, pagination_config, created_at
             FROM api_templates WHERE name = ?1"
        )?;
        stmt.query_row([name], Self::row_to_template).optional().map_err(Into::into)
    }

    pub fn list_templates(&self) -> Result<Vec<ApiTemplate>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, jsonpath_title, jsonpath_description, jsonpath_date, jsonpath_url, jsonpath_source, pagination_config, created_at
             FROM api_templates ORDER BY name"
        )?;
        let rows = stmt.query_map([], |row| Self::row_to_template(row))?;
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
            created_at: parse_ts(&created).unwrap_or_else(|| Utc::now()),
        })
    }

    // ── Keywords ────────────────────────────────────────────────────────────

    pub fn create_keyword(&self, kw: &KeywordCreate) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO keywords (pattern, is_regex, case_sensitive, criticality, enabled)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                kw.pattern, kw.is_regex as i64, kw.case_sensitive as i64,
                format!("{:?}", kw.criticality), kw.enabled as i64
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_keyword(&self, id: i64) -> Result<Option<Keyword>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, pattern, is_regex, case_sensitive, criticality, enabled, created_at FROM keywords WHERE id = ?1"
        )?;
        stmt.query_row([id], Self::row_to_keyword).optional().map_err(Into::into)
    }

    pub fn list_keywords(&self, enabled_only: bool) -> Result<Vec<Keyword>> {
        let sql = if enabled_only {
            "SELECT id, pattern, is_regex, case_sensitive, criticality, enabled, created_at FROM keywords WHERE enabled = 1 ORDER BY id"
        } else {
            "SELECT id, pattern, is_regex, case_sensitive, criticality, enabled, created_at FROM keywords ORDER BY id"
        };
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map([], |row| Self::row_to_keyword(row))?;
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
        self.conn.execute("DELETE FROM keywords WHERE id = ?1", [id])?;
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
            created_at: parse_ts(&created).unwrap_or_else(|| Utc::now()),
        })
    }

    // ── Alerts ────────────────────────────────────────────────────────────

    pub fn create_alert(&self, alert: &AlertCreate) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO alerts (feed_id, keyword_id, title, content_snippet, criticality, content_hash, metadata_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                alert.feed_id, alert.keyword_id, alert.title, alert.content_snippet,
                format!("{:?}", alert.criticality), alert.content_hash, alert.metadata_json
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_alert(&self, id: i64) -> Result<Option<Alert>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, feed_id, keyword_id, title, content_snippet, criticality, read, content_hash, detected_at, metadata_json
             FROM alerts WHERE id = ?1"
        )?;
        stmt.query_row([id], Self::row_to_alert).optional().map_err(Into::into)
    }

    pub fn list_alerts(&self, filter: &AlertFilter) -> Result<Vec<AlertWithMeta>> {
        let mut conditions = Vec::new();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(crit) = &filter.criticality {
            conditions.push(format!("a.criticality = '{}'{}", format!("{:?}", crit), ""));
        }
        if filter.unread_only {
            conditions.push("a.read = 0".to_string());
        }
        if let Some(tag_id) = filter.tag_id {
            conditions.push(format!("a.id IN (SELECT alert_id FROM alert_tags WHERE tag_id = {})", tag_id));
        }
        if let Some(feed_id) = filter.feed_id {
            conditions.push(format!("a.feed_id = {}", feed_id));
        }
        if let Some(keyword_id) = filter.keyword_id {
            conditions.push(format!("a.keyword_id = {}", keyword_id));
        }
        if let Some(text) = &filter.text {
            if !text.is_empty() {
                conditions.push("(a.content_snippet LIKE ?1 OR a.title LIKE ?1 OR f.name LIKE ?1 OR k.pattern LIKE ?1)".to_string());
                params_vec.push(Box::new(format!("%{}%", text)));
            }
        }

        let where_clause = if conditions.is_empty() {
            "".to_string()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            "SELECT a.id, a.feed_id, a.keyword_id, a.title, a.content_snippet, a.criticality, a.read, a.content_hash, a.detected_at, a.metadata_json,
                    f.name as feed_name, k.pattern as keyword_pattern
             FROM alerts a
             JOIN feeds f ON a.feed_id = f.id
             JOIN keywords k ON a.keyword_id = k.id
             {} ORDER BY a.detected_at DESC LIMIT ?1",
            where_clause
        );

        let limit = filter.limit.unwrap_or(500);
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([&limit as &dyn rusqlite::ToSql], Self::row_to_alert_with_meta)?;
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
        self.conn.execute(
            "UPDATE alerts SET read = ?1",
            [read as i64],
        )?;
        Ok(())
    }

    pub fn delete_alert(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM alerts WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn delete_alerts_by_ids(&self, ids: &[i64]) -> Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }
        let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
        let sql = format!("DELETE FROM alerts WHERE id IN ({})", placeholders.join(","));
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

    pub fn get_alert_count(&self) -> Result<i64> {
        let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM alerts")?;
        let count: i64 = stmt.query_row([], |row| row.get(0))?;
        Ok(count)
    }

    pub fn get_unread_alert_count(&self) -> Result<i64> {
        let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM alerts WHERE read = 0")?;
        let count: i64 = stmt.query_row([], |row| row.get(0))?;
        Ok(count)
    }

    pub fn alert_exists_by_hash_window(&self, hash: &str, window: Duration) -> Result<bool> {
        let since = (Utc::now() - window).to_rfc3339();
        let mut stmt = self.conn.prepare(
            "SELECT 1 FROM alerts WHERE content_hash = ?1 AND detected_at > ?2 LIMIT 1"
        )?;
        let exists = stmt.query_row(params![hash, since], |_row| Ok(())).optional()?.is_some();
        Ok(exists)
    }

    pub fn get_criticality_distribution(&self) -> Result<Vec<(Criticality, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT criticality, COUNT(*) FROM alerts GROUP BY criticality ORDER BY criticality"
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
             GROUP BY a.keyword_id ORDER BY cnt DESC LIMIT ?1"
        )?;
        let rows = stmt.query_map([limit as i64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_alert_trend(&self, days: u32) -> Result<Vec<(String, i64)>> {
        let since = (Utc::now() - Duration::days(days as i64)).format("%Y-%m-%d").to_string();
        let mut stmt = self.conn.prepare(
            "SELECT DATE(detected_at) as day, COUNT(*) as cnt FROM alerts
             WHERE detected_at > ?1 GROUP BY day ORDER BY day"
        )?;
        let rows = stmt.query_map([since], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn row_to_alert(row: &rusqlite::Row) -> rusqlite::Result<Alert> {
        let criticality_str: String = row.get(5)?;
        let detected_str: String = row.get(8)?;
        Ok(Alert {
            id: row.get(0)?,
            feed_id: row.get(1)?,
            keyword_id: row.get(2)?,
            title: row.get(3)?,
            content_snippet: row.get(4)?,
            criticality: Criticality::from(criticality_str.as_str()),
            read: row.get::<_, i64>(6)? != 0,
            content_hash: row.get(7)?,
            detected_at: parse_ts(&detected_str).unwrap_or_else(|| Utc::now()),
            metadata_json: row.get(9)?,
        })
    }

    fn row_to_alert_with_meta(row: &rusqlite::Row) -> rusqlite::Result<AlertWithMeta> {
        let alert = Self::row_to_alert(row)?;
        let feed_name: String = row.get(10)?;
        let keyword_pattern: String = row.get(11)?;
        Ok(AlertWithMeta {
            alert,
            feed_name,
            keyword_pattern,
            tags: Vec::new(), // populated separately if needed
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
        let mut stmt = self.conn.prepare("SELECT id, name, color, description, created_at FROM tags WHERE id = ?1")?;
        stmt.query_row([id], Self::row_to_tag).optional().map_err(Into::into)
    }

    pub fn list_tags(&self) -> Result<Vec<Tag>> {
        let mut stmt = self.conn.prepare("SELECT id, name, color, description, created_at FROM tags ORDER BY name")?;
        let rows = stmt.query_map([], |row| Self::row_to_tag(row))?;
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
        let rows = stmt.query_map([feed_id], |row| Self::row_to_tag(row))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_keyword_tags(&self, keyword_id: i64) -> Result<Vec<Tag>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.name, t.color, t.description, t.created_at
             FROM tags t JOIN keyword_tags kt ON t.id = kt.tag_id WHERE kt.keyword_id = ?1 ORDER BY t.name"
        )?;
        let rows = stmt.query_map([keyword_id], |row| Self::row_to_tag(row))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_alert_tags(&self, alert_id: i64) -> Result<Vec<Tag>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.name, t.color, t.description, t.created_at
             FROM tags t JOIN alert_tags at ON t.id = at.tag_id WHERE at.alert_id = ?1 ORDER BY t.name"
        )?;
        let rows = stmt.query_map([alert_id], |row| Self::row_to_tag(row))?;
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

    fn row_to_tag(row: &rusqlite::Row) -> rusqlite::Result<Tag> {
        let created: String = row.get(4)?;
        Ok(Tag {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
            description: row.get(3)?,
            created_at: parse_ts(&created).unwrap_or_else(|| Utc::now()),
        })
    }

    // ── Notifications ─────────────────────────────────────────────────────

    pub fn create_notification(&self, cfg: &NotificationCreate) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO notifications (name, channel, config_json, enabled, min_criticality)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                cfg.name, format!("{:?}", cfg.channel), cfg.config_json,
                cfg.enabled as i64, format!("{:?}", cfg.min_criticality)
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_notifications(&self) -> Result<Vec<NotificationConfig>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, channel, config_json, enabled, min_criticality, created_at FROM notifications ORDER BY name"
        )?;
        let rows = stmt.query_map([], |row| Self::row_to_notification(row))?;
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
        self.conn.execute("DELETE FROM notifications WHERE id = ?1", [id])?;
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
            created_at: parse_ts(&created).unwrap_or_else(|| Utc::now()),
        })
    }

    // ── Health Logs ───────────────────────────────────────────────────────

    pub fn add_health_log(&self, feed_id: i64, status: FeedStatus, error: Option<&str>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO feed_health_logs (feed_id, status, error_message) VALUES (?1, ?2, ?3)",
            params![feed_id, format!("{:?}", status), error],
        )?;
        Ok(())
    }

    pub fn get_health_logs(&self, feed_id: Option<i64>, limit: usize) -> Result<Vec<FeedHealthLog>> {
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
        let rows = stmt.query_map(rusqlite::params_from_iter(param_refs), |row| Self::row_to_health_log(row))?;
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
            checked_at: parse_ts(&checked_str).unwrap_or_else(|| Utc::now()),
        })
    }

    // ── Stats ───────────────────────────────────────────────────────────────

    pub fn get_stats(&self) -> Result<Stats> {
        let total_feeds: i64 = self.conn.query_row("SELECT COUNT(*) FROM feeds", [], |row| row.get(0))?;
        let total_alerts: i64 = self.conn.query_row("SELECT COUNT(*) FROM alerts", [], |row| row.get(0))?;
        let total_keywords: i64 = self.conn.query_row("SELECT COUNT(*) FROM keywords", [], |row| row.get(0))?;
        let unread_alerts: i64 = self.conn.query_row("SELECT COUNT(*) FROM alerts WHERE read = 0", [], |row| row.get(0))?;
        let healthy_feeds: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM feeds WHERE enabled = 1 AND consecutive_failures = 0", [], |row| row.get(0)
        )?;
        let warning_feeds: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM feeds WHERE enabled = 1 AND consecutive_failures BETWEEN 1 AND 2", [], |row| row.get(0)
        )?;
        let error_feeds: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM feeds WHERE enabled = 1 AND consecutive_failures >= 3", [], |row| row.get(0)
        )?;
        let disabled_feeds: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM feeds WHERE enabled = 0", [], |row| row.get(0)
        )?;
        Ok(Stats {
            total_feeds, total_alerts, total_keywords, unread_alerts,
            healthy_feeds, warning_feeds, error_feeds, disabled_feeds,
        })
    }

    pub fn get_feed_health_ratio(&self) -> Result<f64> {
        let total: i64 = self.conn.query_row("SELECT COUNT(*) FROM feeds WHERE enabled = 1", [], |row| row.get(0))?;
        if total == 0 {
            return Ok(1.0);
        }
        let healthy: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM feeds WHERE enabled = 1 AND consecutive_failures = 0", [], |row| row.get(0)
        )?;
        Ok(healthy as f64 / total as f64)
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

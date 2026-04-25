use crate::db::AlertCreate;
use crate::db::Db;
use crate::types::*;
use anyhow::Result;
use sha2::{Digest, Sha256};

pub struct AlertEngine;

impl AlertEngine {
    pub fn process_feed_result(
        db: &Db,
        feed: &Feed,
        result: &FeedResult,
        keywords: &[Keyword],
    ) -> Result<Vec<Alert>> {
        let mut created = Vec::new();
        db.store_feed_result_items(feed, result)?;

        for item in &result.items {
            let content = format!(
                "{} {} {} {}",
                item.title.as_deref().unwrap_or(""),
                item.description.as_deref().unwrap_or(""),
                item.url.as_deref().unwrap_or(""),
                item.source.as_deref().unwrap_or("")
            );

            let matches = crate::keyword::KeywordEngine::check_content(&content, keywords);

            for m in matches {
                let hash_input = format!(
                    "{}:{}:{}:{}",
                    feed.id,
                    m.keyword_id,
                    item.title.as_deref().unwrap_or(""),
                    &content
                );
                let mut hasher = Sha256::new();
                hasher.update(hash_input.as_bytes());
                let content_hash = hex::encode(hasher.finalize());

                // Deduplication check
                if db.alert_exists_by_hash_window(&content_hash, chrono::Duration::hours(1))? {
                    continue;
                }

                let snippet = truncate_chars(&content, 200);

                let alert = AlertCreate {
                    feed_id: feed.id,
                    keyword_id: m.keyword_id,
                    title: item.title.clone(),
                    content_snippet: snippet,
                    criticality: m.criticality,
                    content_hash,
                    metadata_json: item.raw_json.clone(),
                };

                let alert_id = db.create_alert(&alert)?;
                if let Ok(Some(a)) = db.get_alert(alert_id) {
                    created.push(a);
                }
            }
        }

        Ok(created)
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}...", truncated)
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::FeedCreate;

    fn temp_db_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("threatdeck-{}-{}.db", name, std::process::id()))
    }

    #[test]
    fn processing_feed_result_stores_items_even_without_keyword_matches() {
        let path = temp_db_path("alert-engine-items");
        let _ = std::fs::remove_file(&path);
        let db = Db::open(&path).unwrap();
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
            raw_content: String::new(),
            items: vec![FetchedFeedItem {
                title: Some("Stored by processor".into()),
                description: Some("Body".into()),
                date: None,
                url: Some("https://example.com/processor".into()),
                source: None,
                raw_json: None,
            }],
        };

        let alerts = AlertEngine::process_feed_result(&db, &feed, &result, &[]).unwrap();
        assert!(alerts.is_empty());
        let items = db.list_feed_items(&FeedItemFilter::default()).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].item.title, "Stored by processor");

        drop(db);
        let _ = std::fs::remove_file(path);
    }
}

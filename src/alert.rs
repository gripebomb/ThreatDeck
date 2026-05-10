use crate::config::AppConfig;
use crate::db::AlertCreate;
use crate::db::Db;
use crate::types::*;
use anyhow::Result;
use sentinel_ioc::{extract_indicators, ExtractionField, ExtractionInput};
use sha2::{Digest, Sha256};

pub struct AlertEngine;

impl AlertEngine {
    pub fn process_feed_result(
        db: &Db,
        feed: &Feed,
        result: &FeedResult,
        keywords: &[Keyword],
    ) -> Result<Vec<Alert>> {
        Self::process_feed_result_with_config(db, feed, result, keywords, &AppConfig::default())
    }

    pub fn process_feed_result_with_config(
        db: &Db,
        feed: &Feed,
        result: &FeedResult,
        keywords: &[Keyword],
        config: &AppConfig,
    ) -> Result<Vec<Alert>> {
        let mut created = Vec::new();
        let stored_items = db.store_feed_result_items_with_ids(feed, result)?;

        for (index, item) in result.items.iter().enumerate() {
            let stored_item = stored_items.get(index).copied();
            let content_indicator_ids = if config.ioc.enabled
                && stored_item
                    .map(|stored_item| stored_item.inserted)
                    .unwrap_or(false)
            {
                let indicators = extract_indicators(&ExtractionInput {
                    content_item_id: stored_item.map(|stored_item| stored_item.id),
                    alert_id: None,
                    feed_id: Some(feed.id),
                    fields: extraction_fields_for_item(item, config.ioc.extract_from_raw_json),
                });
                let indicators =
                    limit_indicators(indicators, config.ioc.max_indicators_per_content_item);
                let indicator_ids = db.store_extracted_indicators(
                    &indicators,
                    None,
                    stored_item.map(|stored_item| stored_item.id),
                    Some(feed.id),
                )?;
                if config.enrichment.enabled && !config.enrichment.enrich_only_alert_indicators {
                    db.queue_enrichment_jobs_for_indicators(&indicator_ids)?;
                }
                indicator_ids
            } else {
                Vec::new()
            };

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
                if config.ioc.enabled && content_indicator_ids.is_empty() {
                    let indicators = extract_indicators(&ExtractionInput {
                        content_item_id: stored_item.map(|stored_item| stored_item.id),
                        alert_id: Some(alert_id),
                        feed_id: Some(feed.id),
                        fields: extraction_fields_for_item(item, config.ioc.extract_from_raw_json),
                    });
                    let indicators =
                        limit_indicators(indicators, config.ioc.max_indicators_per_content_item);
                    let indicator_ids = db.store_extracted_indicators(
                        &indicators,
                        Some(alert_id),
                        stored_item.map(|stored_item| stored_item.id),
                        Some(feed.id),
                    )?;
                    if config.enrichment.enabled {
                        db.queue_enrichment_jobs_for_indicators(&indicator_ids)?;
                    }
                } else {
                    for indicator_id in &content_indicator_ids {
                        db.link_indicator_to_alert(alert_id, *indicator_id)?;
                    }
                    if config.enrichment.enabled {
                        db.queue_enrichment_jobs_for_indicators(&content_indicator_ids)?;
                    }
                }

                if let Ok(Some(a)) = db.get_alert(alert_id) {
                    created.push(a);
                }
            }
        }

        Ok(created)
    }
}

fn extraction_fields_for_item(
    item: &FetchedFeedItem,
    include_raw_json: bool,
) -> Vec<ExtractionField<'_>> {
    let mut fields = Vec::new();
    if let Some(title) = item.title.as_deref() {
        fields.push(ExtractionField {
            name: "title",
            text: title,
        });
    }
    if let Some(description) = item.description.as_deref() {
        fields.push(ExtractionField {
            name: "description",
            text: description,
        });
    }
    if let Some(url) = item.url.as_deref() {
        fields.push(ExtractionField {
            name: "url",
            text: url,
        });
    }
    if let Some(source) = item.source.as_deref() {
        fields.push(ExtractionField {
            name: "source",
            text: source,
        });
    }
    if include_raw_json {
        if let Some(raw_json) = item.raw_json.as_deref() {
            fields.push(ExtractionField {
                name: "metadata_json",
                text: raw_json,
            });
        }
    }
    fields
}

fn limit_indicators(
    indicators: Vec<sentinel_ioc::ExtractedIndicator>,
    limit: usize,
) -> Vec<sentinel_ioc::ExtractedIndicator> {
    if limit == 0 {
        Vec::new()
    } else {
        indicators.into_iter().take(limit).collect()
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
                description: Some("Body mentions CVE-2025-44444".into()),
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
        let indicators = db
            .list_indicators_for_content_item(items[0].item.id)
            .unwrap();
        let normalized = indicators
            .iter()
            .map(|indicator| indicator.normalized_value.as_str())
            .collect::<Vec<_>>();
        assert!(normalized.contains(&"CVE-2025-44444"));
        assert!(normalized.contains(&"https://example.com/processor"));

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn processing_feed_result_extracts_and_links_alert_indicators() {
        let path = temp_db_path("alert-engine-iocs");
        let _ = std::fs::remove_file(&path);
        let db = Db::open(&path).unwrap();
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
            .create_keyword(&crate::db::KeywordCreate {
                pattern: "ransomware".into(),
                is_regex: false,
                case_sensitive: false,
                criticality: Criticality::High,
                enabled: true,
            })
            .unwrap();
        let feed = db.get_feed(feed_id).unwrap().unwrap();
        let keywords = vec![db.get_keyword(keyword_id).unwrap().unwrap()];
        let result = FeedResult {
            content_hash: "feed-hash-iocs".into(),
            raw_content: String::new(),
            items: vec![FetchedFeedItem {
                title: Some("Ransomware mentions CVE-2025-12345".into()),
                description: Some("Payload hash e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 posted at http://Bad.Example.NET/drop".into()),
                date: None,
                url: Some("https://Bad.Example.NET/report".into()),
                source: None,
                raw_json: None,
            }],
        };

        let alerts = AlertEngine::process_feed_result(&db, &feed, &result, &keywords).unwrap();
        assert_eq!(alerts.len(), 1);

        let indicators = db.list_indicators_for_alert(alerts[0].id).unwrap();
        let normalized = indicators
            .iter()
            .map(|indicator| indicator.normalized_value.as_str())
            .collect::<Vec<_>>();
        assert!(normalized.contains(&"CVE-2025-12345"));
        assert!(normalized
            .contains(&"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"));
        assert!(normalized.contains(&"http://bad.example.net/drop"));
        assert!(normalized.contains(&"https://bad.example.net/report"));
        let cve = indicators
            .iter()
            .find(|indicator| indicator.normalized_value == "CVE-2025-12345")
            .unwrap();
        assert_eq!(cve.sighting_count, 1);

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn processing_feed_result_queues_enrichment_for_eligible_indicators() {
        let path = temp_db_path("alert-engine-enrichment-queue");
        let _ = std::fs::remove_file(&path);
        let db = Db::open(&path).unwrap();
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
            .create_keyword(&crate::db::KeywordCreate {
                pattern: "ransomware".into(),
                is_regex: false,
                case_sensitive: false,
                criticality: Criticality::High,
                enabled: true,
            })
            .unwrap();
        db.create_enrichment_provider(&crate::db::EnrichmentProviderCreate {
            name: "cisa-kev".into(),
            provider_type: "cisa_kev".into(),
            enabled: true,
            supports_types: vec![sentinel_ioc::IndicatorType::Cve],
            ..crate::db::EnrichmentProviderCreate::default()
        })
        .unwrap();
        let feed = db.get_feed(feed_id).unwrap().unwrap();
        let keywords = vec![db.get_keyword(keyword_id).unwrap().unwrap()];
        let result = FeedResult {
            content_hash: "feed-hash-enrichment-queue".into(),
            raw_content: String::new(),
            items: vec![FetchedFeedItem {
                title: Some("Ransomware mentions CVE-2025-12345".into()),
                description: Some("New post".into()),
                date: None,
                url: None,
                source: None,
                raw_json: None,
            }],
        };

        let alerts = AlertEngine::process_feed_result(&db, &feed, &result, &keywords).unwrap();
        assert_eq!(alerts.len(), 1);

        let jobs = db.claim_next_enrichment_jobs(10).unwrap();
        assert_eq!(jobs.len(), 1);

        drop(db);
        let _ = std::fs::remove_file(path);
    }
}

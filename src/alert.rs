use anyhow::Result;
use sha2::{Sha256, Digest};
use crate::db::AlertCreate;
use crate::types::*;
use crate::db::Db;

pub struct AlertEngine;

impl AlertEngine {
    pub fn process_feed_result(db: &Db, feed: &Feed, result: &FeedResult, keywords: &[Keyword]) -> Result<Vec<Alert>> {
        let mut created = Vec::new();
        
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
                let hash_input = format!("{}:{}:{}:{}", feed.id, m.keyword_id, item.title.as_deref().unwrap_or(""), &content);
                let mut hasher = Sha256::new();
                hasher.update(hash_input.as_bytes());
                let content_hash = hex::encode(hasher.finalize());
                
                // Deduplication check
                if db.alert_exists_by_hash_window(&content_hash, chrono::Duration::hours(1))? {
                    continue;
                }
                
                let snippet = if content.len() > 200 {
                    format!("{}...", &content[..200])
                } else {
                    content.clone()
                };
                
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

use crate::types::{ApiTemplate, Feed, FeedResult, FetchedFeedItem};
use anyhow::{Context, Result};

pub struct ApiFetcher {
    pub template: Option<ApiTemplate>,
}

impl ApiFetcher {
    pub fn new(template: Option<ApiTemplate>) -> Self {
        Self { template }
    }
}

impl crate::feed::FeedFetcher for ApiFetcher {
    fn fetch(&self, feed: &Feed) -> Result<FeedResult> {
        let mut request = ureq::get(&feed.url);

        if let Some(key) = &feed.api_key {
            request = request.set("Authorization", &format!("Bearer {}", key));
            request = request.set("X-API-Key", key);
        }

        if let Some(headers_json) = &feed.custom_headers {
            if let Ok(headers) = serde_json::from_str::<serde_json::Value>(headers_json) {
                if let Some(obj) = headers.as_object() {
                    for (k, v) in obj {
                        if let Some(val) = v.as_str() {
                            request = request.set(k, val);
                        }
                    }
                }
            }
        }

        let response = request.call().context("API feed request failed")?;
        let body = response
            .into_string()
            .context("reading API response body")?;
        let json: serde_json::Value = serde_json::from_str(&body).context("parsing API JSON")?;

        let mut items = Vec::new();

        // Use template if available
        if let Some(ref template) = self.template {
            items =
                crate::template::TemplateEngine::extract_items(&json, template).unwrap_or_default();
        }

        // Fallback: try common field names
        if items.is_empty() {
            let data_array = if let Some(arr) = json.as_array() {
                arr.clone()
            } else {
                let keys = vec![
                    "items", "data", "results", "posts", "entries", "feeds", "records",
                ];
                let mut found = None;
                for key in &keys {
                    if let Some(arr) = json.get(key).and_then(|v| v.as_array()) {
                        found = Some(arr.clone());
                        break;
                    }
                }
                found.unwrap_or_else(|| vec![json.clone()])
            };

            for item in data_array {
                items.push(FetchedFeedItem {
                    title: item
                        .get("title")
                        .or_else(|| item.get("name"))
                        .or_else(|| item.get("post_title"))
                        .and_then(|v| v.as_str().map(String::from)),
                    description: item
                        .get("description")
                        .or_else(|| item.get("summary"))
                        .or_else(|| item.get("content"))
                        .and_then(|v| v.as_str().map(String::from)),
                    url: item
                        .get("url")
                        .or_else(|| item.get("link"))
                        .or_else(|| item.get("source"))
                        .and_then(|v| v.as_str().map(String::from)),
                    source: item
                        .get("source")
                        .or_else(|| item.get("group"))
                        .or_else(|| item.get("group_name"))
                        .and_then(|v| v.as_str().map(String::from)),
                    date: item
                        .get("date")
                        .or_else(|| item.get("published"))
                        .or_else(|| item.get("discovered"))
                        .or_else(|| item.get("pubDate"))
                        .and_then(|v| v.as_str().and_then(|s| parse_date(s))),
                    raw_json: Some(item.to_string()),
                });
            }
        }

        let hash = crate::feed::utils::hash_content(&body);
        Ok(FeedResult {
            content_hash: hash,
            items,
            raw_content: body,
        })
    }
}

fn parse_date(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .or_else(|| {
            chrono::DateTime::parse_from_rfc2822(s)
                .ok()
                .map(|dt| dt.with_timezone(&chrono::Utc))
        })
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|dt| chrono::DateTime::from_naive_utc_and_offset(dt, chrono::Utc))
        })
        .or_else(|| {
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .map(|dt| chrono::DateTime::from_naive_utc_and_offset(dt, chrono::Utc))
        })
}

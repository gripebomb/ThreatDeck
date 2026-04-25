use crate::types::{ApiTemplate, FetchedFeedItem};
use anyhow::{bail, Result};

pub struct TemplateEngine;

impl TemplateEngine {
    pub fn extract_items(
        json: &serde_json::Value,
        template: &ApiTemplate,
    ) -> Result<Vec<FetchedFeedItem>> {
        let mut items = Vec::new();

        // Try to find an array in the JSON
        let array = if let Some(arr) = json.as_array() {
            arr.clone()
        } else {
            // Look for common array keys
            let keys = vec!["items", "data", "results", "posts", "entries", "feeds"];
            let mut found = None;
            for key in &keys {
                if let Some(arr) = json.get(key).and_then(|v| v.as_array()) {
                    found = Some(arr.clone());
                    break;
                }
            }
            found.unwrap_or_else(|| vec![json.clone()])
        };

        for item in array {
            let title = Self::extract_jsonpath(&item, &template.jsonpath_title);
            let description = Self::extract_jsonpath(&item, &template.jsonpath_description);
            let url = Self::extract_jsonpath(&item, &template.jsonpath_url);
            let source = Self::extract_jsonpath(&item, &template.jsonpath_source);
            let date_str = Self::extract_jsonpath(&item, &template.jsonpath_date);
            let date = date_str.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .or_else(|| {
                        chrono::DateTime::parse_from_rfc2822(&s)
                            .ok()
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                    })
                    .or_else(|| {
                        chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S")
                            .ok()
                            .map(|dt| chrono::DateTime::from_naive_utc_and_offset(dt, chrono::Utc))
                    })
            });

            items.push(FetchedFeedItem {
                title,
                description,
                url,
                source,
                date,
                raw_json: Some(item.to_string()),
            });
        }

        Ok(items)
    }

    fn extract_jsonpath(value: &serde_json::Value, path: &str) -> Option<String> {
        if path == "$" {
            return value.as_str().map(String::from);
        }

        let mut current = value;
        let parts: Vec<&str> = path
            .trim_start_matches('$')
            .split('.')
            .filter(|s| !s.is_empty())
            .collect();

        for part in &parts {
            if let Some(idx_str) = part.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                if let Ok(idx) = idx_str.parse::<usize>() {
                    current = current.get(idx)?;
                }
            } else {
                current = current.get(part)?;
            }
        }

        current.as_str().map(String::from)
    }

    pub fn validate_template(template: &ApiTemplate) -> Result<()> {
        for path in &[
            &template.jsonpath_title,
            &template.jsonpath_description,
            &template.jsonpath_date,
            &template.jsonpath_url,
        ] {
            if !path.starts_with('$') {
                bail!("JSONPath must start with $: {}", path);
            }
        }
        Ok(())
    }
}

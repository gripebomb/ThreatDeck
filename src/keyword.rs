use crate::types::{Keyword, MatchResult};
use anyhow::Result;
use regex::Regex;

pub struct KeywordEngine;

impl KeywordEngine {
    pub fn check_content(content: &str, keywords: &[Keyword]) -> Vec<MatchResult> {
        let mut results = Vec::new();
        for kw in keywords {
            if !kw.enabled {
                continue;
            }
            let matches = if kw.is_regex {
                Self::match_regex(content, kw)
            } else {
                Self::match_simple(content, kw)
            };
            results.extend(matches);
        }
        results
    }

    fn match_simple(content: &str, kw: &Keyword) -> Vec<MatchResult> {
        let mut results = Vec::new();
        let search_content = if kw.case_sensitive {
            content.to_string()
        } else {
            content.to_lowercase()
        };
        let pattern = if kw.case_sensitive {
            kw.pattern.clone()
        } else {
            kw.pattern.to_lowercase()
        };

        let mut start = 0;
        while let Some(pos) = search_content[start..].find(&pattern) {
            let absolute_pos = start + pos;
            let matched = &content[absolute_pos..absolute_pos + kw.pattern.len()];
            results.push(MatchResult {
                keyword_id: kw.id,
                pattern: kw.pattern.clone(),
                criticality: kw.criticality,
                matched_text: matched.to_string(),
                position: (absolute_pos, absolute_pos + kw.pattern.len()),
            });
            start = absolute_pos + 1;
        }
        results
    }

    fn match_regex(content: &str, kw: &Keyword) -> Vec<MatchResult> {
        let mut results = Vec::new();
        if let Ok(re) = Self::compile_regex(&kw.pattern, kw.case_sensitive) {
            for mat in re.find_iter(content) {
                results.push(MatchResult {
                    keyword_id: kw.id,
                    pattern: kw.pattern.clone(),
                    criticality: kw.criticality,
                    matched_text: mat.as_str().to_string(),
                    position: (mat.start(), mat.end()),
                });
            }
        }
        results
    }

    pub fn compile_regex(pattern: &str, case_sensitive: bool) -> Result<Regex> {
        let flags = if case_sensitive { "" } else { "(?i)" };
        let full_pattern = format!("{}{}", flags, pattern);
        Regex::new(&full_pattern).map_err(|e| anyhow::anyhow!("Invalid regex: {}", e))
    }
}

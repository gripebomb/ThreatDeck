use anyhow::{Context, Result};
use scraper::{Html, Selector};
use crate::types::{Feed, FeedResult, FeedItem};

pub struct WebFetcher;

impl crate::feed::FeedFetcher for WebFetcher {
    fn fetch(&self, feed: &Feed) -> Result<FeedResult> {
        let body = ureq::get(&feed.url)
            .call()
            .context("Website fetch failed")?
            .into_string()
            .context("reading website body")?;
        
        let document = Html::parse_document(&body);
        let selector = Selector::parse("body").unwrap();
        let text = document.select(&selector)
            .next()
            .map(|el| el.text().collect::<Vec<_>>().join(" "))
            .unwrap_or_default();
        
        let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let hash = crate::feed::utils::hash_content(&cleaned);
        
        let items = vec![FeedItem {
            title: Some(format!("Content from {}", feed.name)),
            description: Some(cleaned.chars().take(500).collect()),
            ..Default::default()
        }];
        
        Ok(FeedResult {
            content_hash: hash,
            items,
            raw_content: body,
        })
    }
}

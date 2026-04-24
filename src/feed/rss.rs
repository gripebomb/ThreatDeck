use anyhow::{Context, Result};
use crate::types::{Feed, FeedResult, FeedItem};
use rss::Channel;

pub struct RssFetcher;

impl crate::feed::FeedFetcher for RssFetcher {
    fn fetch(&self, feed: &Feed) -> Result<FeedResult> {
        let body = ureq::get(&feed.url)
            .call()
            .context("RSS feed request failed")?
            .into_string()
            .context("reading RSS body")?;
        
        let channel = Channel::read_from(body.as_bytes())
            .context("parsing RSS feed")?;
        
        let mut items = Vec::new();
        for item in channel.items() {
            items.push(FeedItem {
                title: item.title().map(String::from),
                description: item.description().map(String::from),
                url: item.link().map(String::from),
                date: item.pub_date().and_then(|s| {
                    chrono::DateTime::parse_from_rfc2822(s).ok().map(|dt| dt.with_timezone(&chrono::Utc))
                }),
                source: Some(channel.title().to_string()),
                ..Default::default()
            });
        }
        
        let hash = crate::feed::utils::hash_content(&body);
        Ok(FeedResult {
            content_hash: hash,
            items,
            raw_content: body,
        })
    }
}

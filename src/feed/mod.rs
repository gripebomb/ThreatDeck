pub mod api;
pub mod rss;
pub mod web;
pub mod onion;
pub mod utils;

use anyhow::Result;
use crate::types::{Feed, FeedResult, FeedType, ApiTemplate};

pub trait FeedFetcher {
    fn fetch(&self, feed: &Feed) -> Result<FeedResult>;
}

pub struct FeedManager;

impl FeedManager {
    pub fn fetch_feed(feed: &Feed, template: Option<ApiTemplate>) -> Result<FeedResult> {
        let fetcher: Box<dyn FeedFetcher> = match feed.feed_type {
            FeedType::Api => Box::new(api::ApiFetcher::new(template)),
            FeedType::Rss => Box::new(rss::RssFetcher),
            FeedType::Website => Box::new(web::WebFetcher),
            FeedType::Onion => Box::new(onion::OnionFetcher),
        };
        fetcher.fetch(feed)
    }
}

use crate::types::{Feed, FeedResult, FetchedFeedItem};
use anyhow::{Context, Result};

/// Onion feed fetcher.
///
/// Note: Direct SOCKS5 proxy support requires a Tor HTTP proxy (e.g., Privoxy).
/// Configure your Tor proxy URL as: http://127.0.0.1:8118 (Privoxy)
/// which forwards to the SOCKS5 proxy at 127.0.0.1:9050.
///
/// To set up Privoxy with Tor:
///   1. Install tor and privoxy
///   2. Add to /etc/privoxy/config: forward-socks5t / 127.0.0.1:9050 .
///   3. Set the tor_proxy field to: http://127.0.0.1:8118
pub struct OnionFetcher;

impl crate::feed::FeedFetcher for OnionFetcher {
    fn fetch(&self, feed: &Feed) -> Result<FeedResult> {
        let proxy_url = feed.tor_proxy.as_deref().unwrap_or("http://127.0.0.1:8118");

        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(120))
            .proxy(ureq::Proxy::new(proxy_url).context("invalid proxy URL")?)
            .build();

        let body = agent
            .get(&feed.url)
            .call()
            .context("Onion site fetch failed — ensure Tor + Privoxy are running")?
            .into_string()
            .context("reading onion site body")?;

        let cleaned = body.split_whitespace().collect::<Vec<_>>().join(" ");
        let hash = crate::feed::utils::hash_content(&cleaned);

        let items = vec![FetchedFeedItem {
            title: Some(format!("Onion content from {}", feed.name)),
            description: Some(cleaned.chars().take(500).collect()),
            date: Some(chrono::Utc::now()),
            ..Default::default()
        }];

        Ok(FeedResult {
            content_hash: hash,
            items,
            raw_content: body,
        })
    }
}

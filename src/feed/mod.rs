pub mod api;
pub mod diagnostics;
pub mod onion;
pub mod rss;
pub mod utils;
pub mod web;

use crate::config::TlsTrustStore;
use crate::types::{ApiTemplate, Feed, FeedResult, FeedType};
use anyhow::Result;

pub trait FeedFetcher {
    fn fetch(&self, feed: &Feed, tls_trust_store: TlsTrustStore) -> Result<FeedResult>;
}

pub struct FeedManager;

pub struct FeedFetchOutcome {
    pub result: Option<FeedResult>,
    pub attempt: diagnostics::FetchAttempt,
}

impl FeedManager {
    pub fn fetch_feed(
        feed: &Feed,
        template: Option<ApiTemplate>,
        tls_trust_store: TlsTrustStore,
    ) -> Result<FeedResult> {
        let fetcher: Box<dyn FeedFetcher> = match feed.feed_type {
            FeedType::Api => Box::new(api::ApiFetcher::new(template)),
            FeedType::Rss => Box::new(rss::RssFetcher),
            FeedType::Website => Box::new(web::WebFetcher),
            FeedType::Onion => Box::new(onion::OnionFetcher),
        };
        fetcher.fetch(feed, tls_trust_store)
    }

    pub fn run_fetch_attempt(
        feed: &Feed,
        template: Option<ApiTemplate>,
        tls_trust_store: TlsTrustStore,
    ) -> FeedFetchOutcome {
        let started = std::time::Instant::now();
        if !feed.url.starts_with("http://") && !feed.url.starts_with("https://") {
            let elapsed_ms = started.elapsed().as_millis();
            let diagnostic = diagnostics::FetchDiagnostic {
                phase: diagnostics::FetchFailurePhase::UrlValidation,
                kind: diagnostics::FetchFailureKind::InvalidUrl,
                summary: "Feed URL is invalid".to_string(),
                detail: Some(feed.url.clone()),
                http_status: None,
                url: feed.url.clone(),
                final_url: None,
                elapsed_ms,
            };
            return FeedFetchOutcome {
                result: None,
                attempt: diagnostics::FetchAttempt {
                    id: None,
                    feed_id: Some(feed.id),
                    attempted_at: None,
                    success: false,
                    url: feed.url.clone(),
                    final_url: None,
                    http_status: None,
                    elapsed_ms,
                    diagnostic: Some(diagnostic),
                    items_seen: None,
                    items_new: None,
                },
            };
        }

        match Self::fetch_feed(feed, template, tls_trust_store) {
            Ok(result) => {
                let elapsed_ms = started.elapsed().as_millis();
                FeedFetchOutcome {
                    attempt: diagnostics::FetchAttempt {
                        id: None,
                        feed_id: Some(feed.id),
                        attempted_at: None,
                        success: true,
                        url: feed.url.clone(),
                        final_url: None,
                        http_status: None,
                        elapsed_ms,
                        diagnostic: None,
                        items_seen: Some(result.items.len()),
                        items_new: None,
                    },
                    result: Some(result),
                }
            }
            Err(error) => {
                let elapsed_ms = started.elapsed().as_millis();
                let diagnostic = diagnostics::classify_anyhow_error(&feed.url, &error, elapsed_ms);
                FeedFetchOutcome {
                    result: None,
                    attempt: diagnostics::FetchAttempt {
                        id: None,
                        feed_id: Some(feed.id),
                        attempted_at: None,
                        success: false,
                        url: feed.url.clone(),
                        final_url: None,
                        http_status: diagnostic.http_status,
                        elapsed_ms,
                        diagnostic: Some(diagnostic),
                        items_seen: None,
                        items_new: None,
                    },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feed::diagnostics::{FetchFailureKind, FetchFailurePhase};

    #[test]
    fn run_fetch_attempt_classifies_invalid_url_without_network() {
        let feed = Feed {
            id: 42,
            name: "Bad".into(),
            url: "not a url".into(),
            feed_type: FeedType::Rss,
            enabled: true,
            interval_secs: 300,
            last_fetch_at: None,
            last_fetch_success_at: None,
            last_fetch_failed_at: None,
            last_error: None,
            last_failure_phase: None,
            last_failure_kind: None,
            last_http_status: None,
            consecutive_failures: 0,
            content_hash: None,
            created_at: chrono::Utc::now(),
            api_template_id: None,
            api_key: None,
            custom_headers: None,
            tor_proxy: None,
        };

        let outcome =
            FeedManager::run_fetch_attempt(&feed, None, crate::config::TlsTrustStore::Bundled);
        assert!(outcome.result.is_none());
        let diagnostic = outcome.attempt.diagnostic.unwrap();
        assert_eq!(diagnostic.phase, FetchFailurePhase::UrlValidation);
        assert_eq!(diagnostic.kind, FetchFailureKind::InvalidUrl);
    }
}

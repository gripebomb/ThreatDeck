use crate::config::TlsTrustStore;
use crate::db::Db;
use crate::types::*;
use anyhow::{Context, Result};
use serde_json::Value;

pub struct NotifyEngine;

impl NotifyEngine {
    pub fn send_for_alert(
        db: &Db,
        alert: &Alert,
        feed: &Feed,
        keyword: &Keyword,
        tls_trust_store: TlsTrustStore,
    ) -> Result<()> {
        let configs = db.list_notifications()?;
        let context = AlertNotificationContext::from_db(db, alert)?;
        for cfg in configs {
            if !cfg.enabled {
                continue;
            }
            if alert.criticality < cfg.min_criticality {
                continue;
            }

            let res = match cfg.channel {
                NotificationChannel::Email => {
                    Self::send_email(&cfg, alert, feed, keyword, &context)
                }
                NotificationChannel::Webhook => {
                    Self::send_webhook(&cfg, alert, feed, keyword, &context, tls_trust_store)
                }
                NotificationChannel::Discord => {
                    Self::send_discord(&cfg, alert, feed, keyword, &context, tls_trust_store)
                }
            };

            if let Err(e) = res {
                eprintln!("Notification failed for {}: {}", cfg.name, e);
            }
        }
        Ok(())
    }

    fn send_email(
        cfg: &NotificationConfig,
        alert: &Alert,
        feed: &Feed,
        keyword: &Keyword,
        context: &AlertNotificationContext,
    ) -> Result<()> {
        let email_cfg: EmailConfig =
            serde_json::from_str(&cfg.config_json).context("parsing email config")?;

        let subject = format!(
            "[ThreatDeck] {} alert from {}",
            alert.criticality, feed.name
        );
        let _body = format!(
            "Alert detected:\n\nFeed: {}\nKeyword: {}\nCriticality: {:?}\n\nContent:\n{}\n\nExtracted IOCs:\n{}\n\nEnrichment:\n{}\n\nDetected: {}",
            feed.name,
            keyword.pattern,
            alert.criticality,
            alert.content_snippet,
            context.indicator_summary(),
            context.enrichment_summary(),
            alert.detected_at
        );

        // Note: lettre integration would go here. For now, log the intent.
        println!("[EMAIL] To: {:?}, Subject: {}", email_cfg.to, subject);
        Ok(())
    }

    fn send_webhook(
        cfg: &NotificationConfig,
        alert: &Alert,
        feed: &Feed,
        keyword: &Keyword,
        context: &AlertNotificationContext,
        tls_trust_store: TlsTrustStore,
    ) -> Result<()> {
        let webhook_cfg: WebhookConfig =
            serde_json::from_str(&cfg.config_json).context("parsing webhook config")?;

        let payload = webhook_payload(alert, feed, keyword, context);

        let agent = crate::http::agent(tls_trust_store)?;
        let mut request = agent.post(&webhook_cfg.url);
        for (k, v) in &webhook_cfg.headers {
            request = request.set(k, v);
        }

        request.send_json(payload).context("webhook POST failed")?;
        Ok(())
    }

    fn send_discord(
        cfg: &NotificationConfig,
        alert: &Alert,
        feed: &Feed,
        keyword: &Keyword,
        context: &AlertNotificationContext,
        tls_trust_store: TlsTrustStore,
    ) -> Result<()> {
        let discord_cfg: DiscordConfig =
            serde_json::from_str(&cfg.config_json).context("parsing discord config")?;

        let payload = discord_payload(alert, feed, keyword, context);

        crate::http::agent(tls_trust_store)?
            .post(&discord_cfg.webhook_url)
            .send_json(payload)
            .context("Discord webhook failed")?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
struct AlertNotificationContext {
    indicators: Vec<NotificationIndicator>,
}

impl AlertNotificationContext {
    fn from_db(db: &Db, alert: &Alert) -> Result<Self> {
        let indicators = db
            .list_indicators_for_alert(alert.id)?
            .into_iter()
            .map(|indicator| {
                let enrichment_results = db
                    .get_latest_enrichment_results(indicator.id)
                    .unwrap_or_default();
                NotificationIndicator {
                    indicator_type: indicator_type_label(indicator.indicator_type).to_string(),
                    value: indicator.normalized_value,
                    reputation: enrichment_reputation_label(&enrichment_results),
                    risk_score: indicator.risk_score,
                }
            })
            .collect();
        Ok(Self { indicators })
    }

    fn indicator_summary(&self) -> String {
        if self.indicators.is_empty() {
            return "No indicators extracted.".to_string();
        }

        let mut counts = std::collections::BTreeMap::new();
        for indicator in &self.indicators {
            *counts
                .entry(indicator.indicator_type.as_str())
                .or_insert(0usize) += 1;
        }
        counts
            .into_iter()
            .map(|(indicator_type, count)| format!("{count} {indicator_type}"))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn enrichment_summary(&self) -> String {
        let highlighted = self
            .indicators
            .iter()
            .filter(|indicator| indicator.reputation != "Unknown")
            .map(|indicator| format!("{}: {}", indicator.value, indicator.reputation))
            .collect::<Vec<_>>();
        if highlighted.is_empty() {
            "No enrichment highlights.".to_string()
        } else {
            highlighted.join("\n")
        }
    }

    fn indicators_json(&self) -> Vec<Value> {
        self.indicators
            .iter()
            .map(|indicator| {
                serde_json::json!({
                    "type": indicator.indicator_type,
                    "value": indicator.value,
                    "reputation": indicator.reputation,
                    "risk_score": indicator.risk_score,
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
struct NotificationIndicator {
    indicator_type: String,
    value: String,
    reputation: String,
    risk_score: Option<i64>,
}

fn webhook_payload(
    alert: &Alert,
    feed: &Feed,
    keyword: &Keyword,
    context: &AlertNotificationContext,
) -> Value {
    serde_json::json!({
            "feed": feed.name,
            "keyword": keyword.pattern,
            "criticality": format!("{:?}", alert.criticality),
            "content": alert.content_snippet,
            "detected_at": alert.detected_at.to_rfc3339(),
            "indicator_summary": context.indicator_summary(),
            "enrichment_summary": context.enrichment_summary(),
            "indicators": context.indicators_json(),
    })
}

fn discord_payload(
    alert: &Alert,
    feed: &Feed,
    keyword: &Keyword,
    context: &AlertNotificationContext,
) -> Value {
    let color = match alert.criticality {
        Criticality::Low => 0x64B5F6,
        Criticality::Medium => 0xFFB74D,
        Criticality::High => 0xFF7043,
        Criticality::Critical => 0xE53935,
    };

    serde_json::json!({
        "content": format!("**ThreatDeck Alert** - {:?} criticality", alert.criticality),
        "embeds": [{
            "title": format!("Alert from {}", feed.name),
            "description": alert.content_snippet,
            "color": color,
            "fields": [
                {"name": "Keyword", "value": keyword.pattern, "inline": true},
                {"name": "Feed", "value": feed.name, "inline": true},
                {"name": "IOCs", "value": context.indicator_summary(), "inline": false},
                {"name": "Enrichment", "value": context.enrichment_summary(), "inline": false},
                {"name": "Detected", "value": alert.detected_at.to_rfc3339(), "inline": false}
            ],
            "timestamp": alert.detected_at.to_rfc3339()
        }]
    })
}

fn enrichment_reputation_label(results: &[crate::db::EnrichmentResultRecord]) -> String {
    results
        .iter()
        .find_map(|result| result.verdict.as_deref())
        .or_else(|| {
            results
                .iter()
                .find_map(|result| result.reputation.as_deref())
        })
        .unwrap_or("Unknown")
        .to_string()
}

fn indicator_type_label(indicator_type: sentinel_ioc::IndicatorType) -> &'static str {
    match indicator_type {
        sentinel_ioc::IndicatorType::Ipv4 => "IPv4",
        sentinel_ioc::IndicatorType::Ipv6 => "IPv6",
        sentinel_ioc::IndicatorType::Domain => "Domain",
        sentinel_ioc::IndicatorType::Url => "URL",
        sentinel_ioc::IndicatorType::Email => "Email",
        sentinel_ioc::IndicatorType::Md5 => "MD5",
        sentinel_ioc::IndicatorType::Sha1 => "SHA1",
        sentinel_ioc::IndicatorType::Sha256 => "SHA256",
        sentinel_ioc::IndicatorType::Cve => "CVE",
        sentinel_ioc::IndicatorType::MitreAttackTechnique => "MITRE",
        sentinel_ioc::IndicatorType::OnionDomain => "Onion",
        sentinel_ioc::IndicatorType::OnionUrl => "Onion URL",
        sentinel_ioc::IndicatorType::CryptoWallet => "Wallet",
        sentinel_ioc::IndicatorType::CloudAccessKey => "Cloud Key",
        sentinel_ioc::IndicatorType::Unknown => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn alert() -> Alert {
        Alert {
            id: 1,
            feed_id: 2,
            keyword_id: 3,
            title: Some("Alert".into()),
            content_snippet: "CVE-2025-12345 found".into(),
            criticality: Criticality::High,
            read: false,
            content_hash: "hash".into(),
            detected_at: Utc::now(),
            metadata_json: None,
            status: crate::types::AlertStatus::New,
            disposition: crate::types::AlertDisposition::Unknown,
            severity_override: None,
            confidence_score: None,
            owner: None,
            triage_notes: None,
            acknowledged_at: None,
            investigating_at: None,
            escalated_at: None,
            closed_at: None,
            closed_reason: None,
        }
    }

    fn feed() -> Feed {
        Feed {
            id: 2,
            name: "Feed".into(),
            url: "https://example.com".into(),
            feed_type: FeedType::Rss,
            enabled: true,
            interval_secs: 300,
            api_template_id: None,
            last_fetch_at: None,
            last_error: None,
            last_fetch_success_at: None,
            last_fetch_failed_at: None,
            last_failure_phase: None,
            last_failure_kind: None,
            last_http_status: None,
            consecutive_failures: 0,
            content_hash: None,
            created_at: Utc::now(),
            api_key: None,
            custom_headers: None,
            tor_proxy: None,
        }
    }

    fn keyword() -> Keyword {
        Keyword {
            id: 3,
            pattern: "CVE".into(),
            is_regex: false,
            case_sensitive: false,
            criticality: Criticality::High,
            enabled: true,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn webhook_payload_includes_ioc_and_enrichment_summaries() {
        let context = AlertNotificationContext {
            indicators: vec![NotificationIndicator {
                indicator_type: "CVE".into(),
                value: "CVE-2025-12345".into(),
                reputation: "Known Exploited".into(),
                risk_score: Some(95),
            }],
        };

        let payload = webhook_payload(&alert(), &feed(), &keyword(), &context);

        assert_eq!(payload["indicator_summary"], "1 CVE");
        assert_eq!(payload["indicators"][0]["reputation"], "Known Exploited");
        assert!(payload["enrichment_summary"]
            .as_str()
            .unwrap()
            .contains("CVE-2025-12345"));
    }
}

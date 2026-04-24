use anyhow::{Context, Result};
use crate::types::*;
use crate::db::Db;

pub struct NotifyEngine;

impl NotifyEngine {
    pub fn send_for_alert(db: &Db, alert: &Alert, feed: &Feed, keyword: &Keyword) -> Result<()> {
        let configs = db.list_notifications()?;
        for cfg in configs {
            if !cfg.enabled {
                continue;
            }
            if alert.criticality < cfg.min_criticality {
                continue;
            }
            
            let res = match cfg.channel {
                NotificationChannel::Email => Self::send_email(&cfg, alert, feed, keyword),
                NotificationChannel::Webhook => Self::send_webhook(&cfg, alert, feed, keyword),
                NotificationChannel::Discord => Self::send_discord(&cfg, alert, feed, keyword),
            };
            
            if let Err(e) = res {
                eprintln!("Notification failed for {}: {}", cfg.name, e);
            }
        }
        Ok(())
    }

    fn send_email(cfg: &NotificationConfig, alert: &Alert, feed: &Feed, keyword: &Keyword) -> Result<()> {
        let email_cfg: EmailConfig = serde_json::from_str(&cfg.config_json)
            .context("parsing email config")?;
        
        let subject = format!("[ThreatStream] {} alert from {}", alert.criticality, feed.name);
        let _body = format!(
            "Alert detected:\n\nFeed: {}\nKeyword: {}\nCriticality: {:?}\n\nContent:\n{}\n\nDetected: {}",
            feed.name, keyword.pattern, alert.criticality, alert.content_snippet, alert.detected_at
        );
        
        // Note: lettre integration would go here. For now, log the intent.
        println!("[EMAIL] To: {:?}, Subject: {}", email_cfg.to, subject);
        Ok(())
    }

    fn send_webhook(cfg: &NotificationConfig, alert: &Alert, feed: &Feed, keyword: &Keyword) -> Result<()> {
        let webhook_cfg: WebhookConfig = serde_json::from_str(&cfg.config_json)
            .context("parsing webhook config")?;
        
        let payload = serde_json::json!({
            "feed": feed.name,
            "keyword": keyword.pattern,
            "criticality": format!("{:?}", alert.criticality),
            "content": alert.content_snippet,
            "detected_at": alert.detected_at.to_rfc3339(),
        });
        
        let mut request = ureq::post(&webhook_cfg.url);
        for (k, v) in &webhook_cfg.headers {
            request = request.set(k, v);
        }
        
        request.send_json(payload).context("webhook POST failed")?;
        Ok(())
    }

    fn send_discord(cfg: &NotificationConfig, alert: &Alert, feed: &Feed, keyword: &Keyword) -> Result<()> {
        let discord_cfg: DiscordConfig = serde_json::from_str(&cfg.config_json)
            .context("parsing discord config")?;
        
        let color = match alert.criticality {
            Criticality::Low => 0x64B5F6,
            Criticality::Medium => 0xFFB74D,
            Criticality::High => 0xFF7043,
            Criticality::Critical => 0xE53935,
        };
        
        let payload = serde_json::json!({
            "content": format!("**ThreatStream Alert** — {:?} criticality", alert.criticality),
            "embeds": [{
                "title": format!("Alert from {}", feed.name),
                "description": alert.content_snippet,
                "color": color,
                "fields": [
                    {"name": "Keyword", "value": keyword.pattern, "inline": true},
                    {"name": "Feed", "value": feed.name, "inline": true},
                    {"name": "Detected", "value": alert.detected_at.to_rfc3339(), "inline": false}
                ],
                "timestamp": alert.detected_at.to_rfc3339()
            }]
        });
        
        ureq::post(&discord_cfg.webhook_url)
            .send_json(payload)
            .context("Discord webhook failed")?;
        Ok(())
    }
}

use crate::db::{AlertFilter, AlertReportData, Db, FeedHealthRecord};
use crate::types::AlertWithMeta;
use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use threatdeck_report::{
    generate_filename, render_alert_collection_report, render_alert_report,
    render_feed_health_report, ReportCount, ReportExportOptions, ReportExportResult,
    ReportFeedHealth, ReportFeedSummary, ReportIndicator, ReportKeywordSummary, ReportTag,
    ReportTriageEvent, ReportType,
};

pub struct ReportService;

impl ReportService {
    pub fn new() -> Self {
        Self
    }

    pub fn export_alert_report(
        &self,
        db: &Db,
        alert_id: i64,
        options: &ReportExportOptions,
        export_dir: &Path,
    ) -> Result<ReportExportResult> {
        let data = db
            .get_alert_report_data(alert_id)?
            .with_context(|| format!("Alert {alert_id} not found"))?;

        let report = self.build_alert_report(&data, options)?;
        let markdown = render_alert_report(&report, options);

        let filename = validated_report_filename(options.output_path.as_ref(), || {
            generate_filename(ReportType::Alert, Some(alert_id), None)
        })?;

        let output_path = export_dir.join(&filename);

        if output_path.exists() && !options.overwrite {
            return Err(threatdeck_report::ReportError::ExportPathExists(output_path).into());
        }

        fs::create_dir_all(export_dir)
            .with_context(|| format!("creating export directory: {}", export_dir.display()))?;

        fs::write(&output_path, &markdown)
            .with_context(|| format!("writing report file: {}", output_path.display()))?;

        let bytes_written = markdown.len() as u64;

        Ok(ReportExportResult {
            report_type: ReportType::Alert,
            format: threatdeck_report::ExportFormat::Markdown,
            path: output_path,
            bytes_written,
            generated_at: Utc::now(),
        })
    }

    pub fn export_visible_alerts_report(
        &self,
        db: &Db,
        alerts: &[AlertWithMeta],
        filter: &AlertFilter,
        options: &ReportExportOptions,
        export_dir: &Path,
    ) -> Result<ReportExportResult> {
        let report = self.build_alert_collection_report(db, alerts, filter, options)?;
        let markdown = render_alert_collection_report(&report);

        let filename = validated_report_filename(options.output_path.as_ref(), || {
            generate_filename(ReportType::AlertCollection, None, None)
        })?;

        let output_path = export_dir.join(&filename);

        if output_path.exists() && !options.overwrite {
            return Err(threatdeck_report::ReportError::ExportPathExists(output_path).into());
        }

        fs::create_dir_all(export_dir)
            .with_context(|| format!("creating export directory: {}", export_dir.display()))?;

        fs::write(&output_path, &markdown)
            .with_context(|| format!("writing report file: {}", output_path.display()))?;

        let bytes_written = markdown.len() as u64;

        Ok(ReportExportResult {
            report_type: ReportType::AlertCollection,
            format: threatdeck_report::ExportFormat::Markdown,
            path: output_path,
            bytes_written,
            generated_at: Utc::now(),
        })
    }

    pub fn export_feed_health_report(
        &self,
        db: &Db,
        options: &ReportExportOptions,
        export_dir: &Path,
    ) -> Result<ReportExportResult> {
        let feeds = db.list_feed_health()?;
        let report = self.build_feed_health_report(&feeds)?;
        let markdown = render_feed_health_report(&report);

        let filename = validated_report_filename(options.output_path.as_ref(), || {
            generate_filename(ReportType::FeedHealth, None, None)
        })?;

        let output_path = export_dir.join(&filename);

        if output_path.exists() && !options.overwrite {
            return Err(threatdeck_report::ReportError::ExportPathExists(output_path).into());
        }

        fs::create_dir_all(export_dir)
            .with_context(|| format!("creating export directory: {}", export_dir.display()))?;

        fs::write(&output_path, &markdown)
            .with_context(|| format!("writing report file: {}", output_path.display()))?;

        let bytes_written = markdown.len() as u64;

        Ok(ReportExportResult {
            report_type: ReportType::FeedHealth,
            format: threatdeck_report::ExportFormat::Markdown,
            path: output_path,
            bytes_written,
            generated_at: Utc::now(),
        })
    }

    fn build_alert_report(
        &self,
        data: &AlertReportData,
        options: &ReportExportOptions,
    ) -> Result<threatdeck_report::AlertReport> {
        let alert = &data.alert;

        let tags: Vec<ReportTag> = data
            .tags
            .iter()
            .map(|t| ReportTag {
                name: t.name.clone(),
                color: t.color.clone(),
            })
            .collect();

        let indicators: Vec<ReportIndicator> = data
            .indicators
            .iter()
            .map(|i| ReportIndicator {
                indicator_type: format!("{:?}", i.indicator_type),
                value: i.value.clone(),
                normalized_value: i.normalized_value.clone(),
                reputation: i.risk_score.map(|_| "Unknown".to_string()),
                risk_score: i.risk_score.map(|s| s as i32),
            })
            .collect();

        let triage_history: Vec<ReportTriageEvent> = data
            .triage_history
            .iter()
            .map(|e| ReportTriageEvent {
                time: e.created_at.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                event_type: e.event_type.clone(),
                old_value: e.old_value.clone(),
                new_value: e.new_value.clone(),
                actor: e.actor.clone(),
                note: e.note.clone(),
            })
            .collect();

        let metadata = if options.include_metadata {
            alert
                .metadata_json
                .as_ref()
                .and_then(|m| serde_json::from_str(m).ok())
                .unwrap_or(serde_json::Value::Null)
        } else {
            serde_json::Value::Null
        };

        let raw_content = if options.include_raw_content {
            alert.metadata_json.clone()
        } else {
            None
        };

        Ok(threatdeck_report::AlertReport {
            alert_id: alert.id,
            title: alert
                .title
                .clone()
                .unwrap_or_else(|| format!("Alert {}", alert.id)),
            summary: None,
            criticality: format!("{:?}", alert.criticality),
            severity: alert.severity_override.map(|s| format!("{:?}", s)),
            confidence_score: alert.confidence_score.map(|s| s as i32),
            status: format!("{:?}", alert.status),
            disposition: format!("{:?}", alert.disposition),
            owner: alert.owner.clone(),
            detected_at: alert
                .detected_at
                .format("%Y-%m-%d %H:%M:%S UTC")
                .to_string(),
            feed: ReportFeedSummary {
                feed_id: alert.feed_id,
                name: data.feed_name.clone(),
                feed_type: data.feed_type.clone(),
                url: data.feed_url.clone(),
            },
            keyword: ReportKeywordSummary {
                keyword_id: alert.keyword_id,
                pattern: data.keyword_pattern.clone(),
                match_type: data.keyword_match_type.clone(),
                criticality: data.keyword_criticality.clone(),
            },
            tags,
            snippet: Some(alert.content_snippet.clone()),
            source_url: None,
            triage_notes: alert.triage_notes.clone(),
            triage_history,
            indicators,
            enrichment: Vec::new(),
            metadata,
            raw_content,
            generated_at: Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        })
    }

    fn build_alert_collection_report(
        &self,
        _db: &Db,
        alerts: &[AlertWithMeta],
        filter: &AlertFilter,
        _options: &ReportExportOptions,
    ) -> Result<threatdeck_report::AlertCollectionReport> {
        let mut filter_summary = Vec::new();

        if filter.unread_only {
            filter_summary.push("Unread alerts only".to_string());
        }
        if let Some(crit) = &filter.criticality {
            filter_summary.push(format!("Criticality: {:?}", crit));
        }
        if let Some(status) = &filter.status {
            filter_summary.push(format!("Status: {:?}", status));
        }
        if let Some(text) = &filter.text {
            if !text.is_empty() {
                filter_summary.push(format!("Search: {}", text));
            }
        }

        let mut criticality_counts: HashMap<String, i64> = HashMap::new();
        let mut status_counts: HashMap<String, i64> = HashMap::new();

        for alert in alerts {
            *criticality_counts
                .entry(format!("{:?}", alert.alert.criticality))
                .or_insert(0) += 1;
            *status_counts
                .entry(format!("{:?}", alert.alert.status))
                .or_insert(0) += 1;
        }

        let counts_by_criticality: Vec<ReportCount> = criticality_counts
            .into_iter()
            .map(|(label, count)| ReportCount { label, count })
            .collect();

        let counts_by_status: Vec<ReportCount> = status_counts
            .into_iter()
            .map(|(label, count)| ReportCount { label, count })
            .collect();

        let alert_summaries: Vec<threatdeck_report::AlertReportSummary> = alerts
            .iter()
            .map(|a| threatdeck_report::AlertReportSummary {
                alert_id: a.alert.id,
                title: a
                    .alert
                    .title
                    .clone()
                    .unwrap_or_else(|| format!("Alert {}", a.alert.id)),
                criticality: format!("{:?}", a.alert.criticality),
                status: format!("{:?}", a.alert.status),
                detected_at: a
                    .alert
                    .detected_at
                    .format("%Y-%m-%d %H:%M:%S UTC")
                    .to_string(),
                feed_name: a.feed_name.clone(),
                keyword_pattern: a.keyword_pattern.clone(),
            })
            .collect();

        Ok(threatdeck_report::AlertCollectionReport {
            title: "ThreatDeck Alert Collection Report".to_string(),
            description: None,
            generated_at: Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            filter_summary,
            total_alerts: alerts.len(),
            counts_by_criticality,
            counts_by_status,
            alerts: alert_summaries,
        })
    }

    fn build_feed_health_report(
        &self,
        feeds: &[FeedHealthRecord],
    ) -> Result<threatdeck_report::FeedHealthReport> {
        let mut healthy = 0;
        let mut warning = 0;
        let mut error = 0;
        let mut disabled = 0;

        for feed in feeds {
            match feed.status.as_str() {
                "Healthy" => healthy += 1,
                "Warning" => warning += 1,
                "Error" => error += 1,
                "Disabled" => disabled += 1,
                _ => {}
            }
        }

        let feed_health: Vec<ReportFeedHealth> = feeds
            .iter()
            .map(|f| ReportFeedHealth {
                feed_id: f.id,
                name: f.name.clone(),
                feed_type: f.feed_type.clone(),
                status: f.status.clone(),
                consecutive_failures: f.consecutive_failures,
                last_fetch_at: f
                    .last_fetch_at
                    .map(|d| d.format("%Y-%m-%d %H:%M:%S UTC").to_string()),
                last_success_at: f
                    .last_fetch_success_at
                    .map(|d| d.format("%Y-%m-%d %H:%M:%S UTC").to_string()),
                last_error: f.last_error.clone(),
            })
            .collect();

        Ok(threatdeck_report::FeedHealthReport {
            generated_at: Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            total_feeds: feeds.len(),
            healthy_feeds: healthy,
            warning_feeds: warning,
            error_feeds: error,
            disabled_feeds: disabled,
            feeds: feed_health,
        })
    }
}
fn validated_report_filename(
    output_path: Option<&std::path::PathBuf>,
    default: impl FnOnce() -> String,
) -> Result<String> {
    let filename = match output_path {
        Some(p) => p
            .file_name()
            .ok_or_else(|| threatdeck_report::ReportError::InvalidExportPath(p.to_path_buf()))?
            .to_string_lossy()
            .to_string(),
        None => default(),
    };

    for component in std::path::Path::new(&filename).components() {
        if !matches!(component, std::path::Component::Normal(_)) {
            return Err(threatdeck_report::ReportError::InvalidExportPath(
                std::path::PathBuf::from(&filename),
            )
            .into());
        }
    }

    Ok(filename)
}

impl Default for ReportService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::ReportService;
    use crate::db::{AlertCreate, AlertFilter, Db, FeedCreate, KeywordCreate};
    use crate::types::{Criticality, FeedType};
    use threatdeck_report::ReportExportOptions;

    fn temp_export_dir(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "threatdeck-report-test-{}-{}",
            name,
            std::process::id()
        ))
    }

    fn seeded_db() -> Db {
        let db = Db::new_in_memory_for_tests();
        db.init_schema().unwrap();
        let feed_id = db
            .create_feed(&FeedCreate {
                name: "Report Feed".into(),
                url: "https://report.test/feed.xml".into(),
                feed_type: FeedType::Rss,
                enabled: true,
                interval_secs: 300,
                ..FeedCreate::default()
            })
            .unwrap();
        let keyword_id = db
            .create_keyword(&KeywordCreate {
                pattern: "breach".into(),
                criticality: Criticality::High,
                enabled: true,
                ..KeywordCreate::default()
            })
            .unwrap();
        db.create_alert(&AlertCreate {
            feed_id,
            keyword_id,
            title: Some("Report test alert".into()),
            content_snippet: "Sensitive data breach content".into(),
            criticality: Criticality::High,
            content_hash: "report-hash".into(),
            metadata_json: None,
        })
        .unwrap();
        db
    }

    fn assert_invalid_export_path(err: anyhow::Error, expected_subpath: &str) {
        let msg = format!("{err:#}");
        assert!(
            msg.contains("Invalid export path"),
            "expected invalid export path error, got: {msg}"
        );
        assert!(
            msg.contains(expected_subpath),
            "expected '{expected_subpath}' in error: {msg}"
        );
    }

    #[test]
    fn report_export_rejects_path_ending_in_dotdot() {
        let db = seeded_db();
        let service = ReportService::new();
        let export_dir = temp_export_dir("dotdir-output");
        let _ = std::fs::remove_dir_all(&export_dir);
        std::fs::create_dir_all(&export_dir).unwrap();
        let dotdot_path: std::path::PathBuf = export_dir.join("..");

        let options = ReportExportOptions {
            output_path: Some(dotdot_path.clone()),
            ..ReportExportOptions::default()
        };

        let err = service
            .export_alert_report(&db, 1, &options, &export_dir)
            .unwrap_err();
        assert_invalid_export_path(err, ".");
    }

    #[test]
    fn report_export_rejects_dotdot_filename_for_alert_collection() {
        let db = seeded_db();
        let service = ReportService::new();
        let export_dir = temp_export_dir("collection-dotdot-output");
        let _ = std::fs::remove_dir_all(&export_dir);
        std::fs::create_dir_all(&export_dir).unwrap();

        let options = ReportExportOptions {
            output_path: Some(std::path::PathBuf::from("..")),
            ..ReportExportOptions::default()
        };
        let filter = AlertFilter::default();
        let alerts = db.list_alerts(&filter).unwrap();

        let err = service
            .export_visible_alerts_report(&db, &alerts, &filter, &options, &export_dir)
            .unwrap_err();
        assert_invalid_export_path(err, "..");
    }

    #[test]
    fn report_export_rejects_dotdot_filename_for_feed_health() {
        let db = seeded_db();
        let service = ReportService::new();
        let export_dir = temp_export_dir("feed-health-dotdot-output");
        let _ = std::fs::remove_dir_all(&export_dir);
        std::fs::create_dir_all(&export_dir).unwrap();

        let options = ReportExportOptions {
            output_path: Some(std::path::PathBuf::from("..")),
            ..ReportExportOptions::default()
        };

        let err = service
            .export_feed_health_report(&db, &options, &export_dir)
            .unwrap_err();
        assert_invalid_export_path(err, "..");
    }

    #[test]
    fn report_export_uses_custom_filename() {
        let db = seeded_db();
        let service = ReportService::new();
        let export_dir = temp_export_dir("custom-filename");
        let _ = std::fs::remove_dir_all(&export_dir);
        std::fs::create_dir_all(&export_dir).unwrap();

        let options = ReportExportOptions {
            output_path: Some(std::path::PathBuf::from("my-report.md")),
            ..ReportExportOptions::default()
        };

        let result = service
            .export_alert_report(&db, 1, &options, &export_dir)
            .unwrap();
        assert_eq!(result.path, export_dir.join("my-report.md"));
        assert!(result.path.exists());
    }

    #[test]
    fn report_export_ignores_directory_components_in_output_path() {
        let db = seeded_db();
        let service = ReportService::new();
        let export_dir = temp_export_dir("nested-filename");
        let _ = std::fs::remove_dir_all(&export_dir);
        std::fs::create_dir_all(&export_dir).unwrap();

        let options = ReportExportOptions {
            output_path: Some(std::path::PathBuf::from("subdir/report.md")),
            ..ReportExportOptions::default()
        };

        let result = service
            .export_alert_report(&db, 1, &options, &export_dir)
            .unwrap();
        assert_eq!(result.path, export_dir.join("report.md"));
        assert!(result.path.exists());
    }
}

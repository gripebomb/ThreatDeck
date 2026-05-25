use chrono::{DateTime, Utc};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportType {
    Alert,
    AlertCollection,
    Case,
    Indicator,
    FeedHealth,
    DailySummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Markdown,
}

#[derive(Debug, Clone)]
pub struct ReportExportOptions {
    pub report_type: ReportType,
    pub format: ExportFormat,
    pub output_path: Option<PathBuf>,
    pub include_raw_content: bool,
    pub include_metadata: bool,
    pub include_iocs: bool,
    pub include_enrichment: bool,
    pub include_triage_history: bool,
    pub include_feed_health: bool,
    pub include_tags: bool,
    pub redact_secrets: bool,
    pub overwrite: bool,
    pub generated_by: Option<String>,
}

impl Default for ReportExportOptions {
    fn default() -> Self {
        Self {
            report_type: ReportType::Alert,
            format: ExportFormat::Markdown,
            output_path: None,
            include_raw_content: false,
            include_metadata: true,
            include_iocs: true,
            include_enrichment: true,
            include_triage_history: true,
            include_feed_health: false,
            include_tags: true,
            redact_secrets: true,
            overwrite: false,
            generated_by: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReportExportResult {
    pub report_type: ReportType,
    pub format: ExportFormat,
    pub path: PathBuf,
    pub bytes_written: u64,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AlertReport {
    pub alert_id: i64,
    pub title: String,
    pub summary: Option<String>,
    pub criticality: String,
    pub severity: Option<String>,
    pub confidence_score: Option<i32>,
    pub status: String,
    pub disposition: String,
    pub owner: Option<String>,
    pub detected_at: String,
    pub feed: ReportFeedSummary,
    pub keyword: ReportKeywordSummary,
    pub tags: Vec<ReportTag>,
    pub snippet: Option<String>,
    pub source_url: Option<String>,
    pub triage_notes: Option<String>,
    pub triage_history: Vec<ReportTriageEvent>,
    pub indicators: Vec<ReportIndicator>,
    pub enrichment: Vec<ReportEnrichmentResult>,
    pub metadata: serde_json::Value,
    pub raw_content: Option<String>,
    pub generated_at: String,
}

#[derive(Debug, Clone)]
pub struct AlertCollectionReport {
    pub title: String,
    pub description: Option<String>,
    pub generated_at: String,
    pub filter_summary: Vec<String>,
    pub total_alerts: usize,
    pub counts_by_criticality: Vec<ReportCount>,
    pub counts_by_status: Vec<ReportCount>,
    pub alerts: Vec<AlertReportSummary>,
}

#[derive(Debug, Clone)]
pub struct CaseReport {
    pub case_id: i64,
    pub title: String,
    pub status: String,
    pub severity: Option<String>,
    pub owner: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub summary: Option<String>,
    pub notes: Vec<ReportNote>,
    pub alerts: Vec<AlertReportSummary>,
    pub indicators: Vec<ReportIndicator>,
    pub timeline: Vec<ReportTimelineEvent>,
    pub generated_at: String,
}

#[derive(Debug, Clone)]
pub struct IndicatorReport {
    pub indicator_id: i64,
    pub indicator_type: String,
    pub value: String,
    pub normalized_value: String,
    pub reputation: Option<String>,
    pub risk_score: Option<i32>,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub sighting_count: i64,
    pub occurrences: Vec<ReportIndicatorOccurrence>,
    pub enrichment: Vec<ReportEnrichmentResult>,
    pub generated_at: String,
}

#[derive(Debug, Clone)]
pub struct FeedHealthReport {
    pub generated_at: String,
    pub total_feeds: usize,
    pub healthy_feeds: usize,
    pub warning_feeds: usize,
    pub error_feeds: usize,
    pub disabled_feeds: usize,
    pub feeds: Vec<ReportFeedHealth>,
}

#[derive(Debug, Clone)]
pub struct DailySummaryReport {
    pub date: String,
    pub generated_at: String,
    pub alert_count: usize,
    pub unread_count: usize,
    pub critical_count: usize,
    pub high_count: usize,
    pub feeds_checked: usize,
    pub failed_feeds: usize,
    pub top_keywords: Vec<ReportCount>,
    pub top_feeds: Vec<ReportCount>,
    pub alerts: Vec<AlertReportSummary>,
}

#[derive(Debug, Clone)]
pub struct ReportFeedSummary {
    pub feed_id: i64,
    pub name: String,
    pub feed_type: String,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct ReportKeywordSummary {
    pub keyword_id: i64,
    pub pattern: String,
    pub match_type: String,
    pub criticality: String,
}

#[derive(Debug, Clone)]
pub struct ReportTag {
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone)]
pub struct ReportIndicator {
    pub indicator_type: String,
    pub value: String,
    pub normalized_value: String,
    pub reputation: Option<String>,
    pub risk_score: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct ReportEnrichmentResult {
    pub provider: String,
    pub verdict: Option<String>,
    pub summary: Option<String>,
    pub reputation: Option<String>,
    pub score: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct ReportTriageEvent {
    pub time: String,
    pub event_type: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub actor: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AlertReportSummary {
    pub alert_id: i64,
    pub title: String,
    pub criticality: String,
    pub status: String,
    pub detected_at: String,
    pub feed_name: String,
    pub keyword_pattern: String,
}

#[derive(Debug, Clone)]
pub struct ReportCount {
    pub label: String,
    pub count: i64,
}

#[derive(Debug, Clone)]
pub struct ReportNote {
    pub author: Option<String>,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct ReportTimelineEvent {
    pub time: String,
    pub event_type: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct ReportIndicatorOccurrence {
    pub alert_id: Option<i64>,
    pub feed_id: Option<i64>,
    pub source_field: Option<String>,
    pub surrounding_text: Option<String>,
    pub detected_at: String,
}

#[derive(Debug, Clone)]
pub struct ReportFeedHealth {
    pub feed_id: i64,
    pub name: String,
    pub feed_type: String,
    pub status: String,
    pub consecutive_failures: i64,
    pub last_fetch_at: Option<String>,
    pub last_success_at: Option<String>,
    pub last_error: Option<String>,
}

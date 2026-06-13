use crate::types::*;

pub fn render_alert_report(report: &AlertReport, options: &ReportExportOptions) -> String {
    let mut out = String::new();

    out.push_str(&format!("# ThreatDeck Alert Report: {}\n\n", report.title));

    out.push_str(
        "> **Handling Notice:** This report may contain sensitive threat intelligence, internal triage notes, indicators, and source metadata. Share only with appropriate recipients.\n\n",
    );

    out.push_str("## Summary\n\n");
    out.push_str("| Field | Value |\n");
    out.push_str("|---|---|\n");
    out.push_str(&format!("| Alert ID | {} |\n", report.alert_id));
    out.push_str(&format!(
        "| Criticality | {} |\n",
        escape_table_cell(&report.criticality)
    ));
    if let Some(severity) = &report.severity {
        out.push_str(&format!("| Severity | {} |\n", escape_table_cell(severity)));
    }
    if let Some(score) = report.confidence_score {
        out.push_str(&format!("| Confidence | {} |\n", score));
    }
    out.push_str(&format!(
        "| Status | {} |\n",
        escape_table_cell(&report.status)
    ));
    out.push_str(&format!(
        "| Disposition | {} |\n",
        escape_table_cell(&report.disposition)
    ));
    if let Some(owner) = &report.owner {
        out.push_str(&format!("| Owner | {} |\n", escape_table_cell(owner)));
    }
    out.push_str(&format!(
        "| Detected | {} |\n",
        escape_table_cell(&report.detected_at)
    ));
    out.push_str(&format!(
        "| Generated | {} |\n",
        escape_table_cell(&report.generated_at)
    ));
    out.push('\n');

    out.push_str("## Source\n\n");
    out.push_str("| Field | Value |\n");
    out.push_str("|---|---|\n");
    out.push_str(&format!(
        "| Feed | {} |\n",
        escape_table_cell(&report.feed.name)
    ));
    out.push_str(&format!(
        "| Feed Type | {} |\n",
        escape_table_cell(&report.feed.feed_type)
    ));
    out.push_str(&format!(
        "| Keyword | {} |\n",
        escape_table_cell(&report.keyword.pattern)
    ));
    if let Some(url) = &report.source_url {
        out.push_str(&format!("| Source URL | {} |\n", escape_table_cell(url)));
    }
    out.push('\n');

    if let Some(snippet) = &report.snippet {
        out.push_str("## Alert Snippet\n\n");
        out.push_str(&format!("> {}\n\n", snippet.replace('\n', " ")));
    }

    if options.include_tags && !report.tags.is_empty() {
        out.push_str("## Tags\n\n");
        for tag in &report.tags {
            out.push_str(&format!("- {}\n", tag.name));
        }
        out.push('\n');
    }

    if options.include_iocs && !report.indicators.is_empty() {
        out.push_str("## Indicators\n\n");
        out.push_str("| Type | Value | Reputation | Risk |\n");
        out.push_str("|---|---|---|---|\n");
        for indicator in &report.indicators {
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                escape_table_cell(&indicator.indicator_type),
                escape_table_cell(&indicator.normalized_value),
                escape_table_cell(indicator.reputation.as_deref().unwrap_or("Unknown")),
                indicator
                    .risk_score
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "-".to_string())
            ));
        }
        out.push('\n');
    }

    if options.include_enrichment && !report.enrichment.is_empty() {
        out.push_str("## Enrichment\n\n");
        for result in &report.enrichment {
            out.push_str(&format!("### {}\n\n", escape_table_cell(&result.provider)));
            out.push_str("| Provider | Verdict | Summary |\n");
            out.push_str("|---|---|---|\n");
            out.push_str(&format!(
                "| {} | {} | {} |\n\n",
                escape_table_cell(&result.provider),
                escape_table_cell(result.verdict.as_deref().unwrap_or("-")),
                escape_table_cell(result.summary.as_deref().unwrap_or("-")),
            ));
        }
    }

    if let Some(notes) = &report.triage_notes {
        out.push_str("## Triage Notes\n\n");
        out.push_str(&format!("{}\n\n", notes));
    }

    if options.include_triage_history && !report.triage_history.is_empty() {
        out.push_str("## Triage History\n\n");
        out.push_str("| Time | Event | Old | New | Actor | Note |\n");
        out.push_str("|---|---|---|---|---|---|\n");
        for event in &report.triage_history {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                escape_table_cell(&event.time),
                escape_table_cell(&event.event_type),
                escape_table_cell(event.old_value.as_deref().unwrap_or("")),
                escape_table_cell(event.new_value.as_deref().unwrap_or("")),
                escape_table_cell(event.actor.as_deref().unwrap_or("")),
                escape_table_cell(event.note.as_deref().unwrap_or("")),
            ));
        }
        out.push('\n');
    }

    if options.include_metadata && !report.metadata.is_null() {
        out.push_str("## Metadata\n\n");
        out.push_str("```json\n");
        out.push_str(&serde_json::to_string_pretty(&report.metadata).unwrap_or_default());
        out.push_str("\n```\n\n");
    }

    if options.include_raw_content {
        if let Some(raw) = &report.raw_content {
            out.push_str("## Raw Content\n\n");
            out.push_str("```\n");
            out.push_str(raw);
            out.push_str("\n```\n\n");
        }
    }

    out
}

pub fn render_alert_collection_report(report: &AlertCollectionReport) -> String {
    let mut out = String::new();

    out.push_str(&format!("# {}\n\n", report.title));

    out.push_str(
        "> **Handling Notice:** This report may contain sensitive threat intelligence, internal triage notes, indicators, and source metadata. Share only with appropriate recipients.\n\n",
    );

    if !report.filter_summary.is_empty() {
        out.push_str("## Filter Summary\n\n");
        for filter in &report.filter_summary {
            out.push_str(&format!("- {}\n", filter));
        }
        out.push('\n');
    }

    out.push_str("## Statistics\n\n");
    out.push_str(&format!("**Total Alerts:** {}\n\n", report.total_alerts));

    if !report.counts_by_criticality.is_empty() {
        out.push_str("### Criticality Counts\n\n");
        out.push_str("| Criticality | Count |\n");
        out.push_str("|---|---|\n");
        for count in &report.counts_by_criticality {
            out.push_str(&format!(
                "| {} | {} |\n",
                escape_table_cell(&count.label),
                count.count
            ));
        }
        out.push('\n');
    }

    if !report.counts_by_status.is_empty() {
        out.push_str("### Status Counts\n\n");
        out.push_str("| Status | Count |\n");
        out.push_str("|---|---|\n");
        for count in &report.counts_by_status {
            out.push_str(&format!(
                "| {} | {} |\n",
                escape_table_cell(&count.label),
                count.count
            ));
        }
        out.push('\n');
    }

    if !report.alerts.is_empty() {
        out.push_str("## Alerts\n\n");
        out.push_str("| ID | Title | Criticality | Status | Detected | Feed | Keyword |\n");
        out.push_str("|---|---|---|---|---|---|---|\n");
        for alert in &report.alerts {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                alert.alert_id,
                escape_table_cell(&alert.title),
                escape_table_cell(&alert.criticality),
                escape_table_cell(&alert.status),
                escape_table_cell(&alert.detected_at),
                escape_table_cell(&alert.feed_name),
                escape_table_cell(&alert.keyword_pattern),
            ));
        }
        out.push('\n');
    }

    out
}

pub fn render_feed_health_report(report: &FeedHealthReport) -> String {
    let mut out = String::new();

    out.push_str("# ThreatDeck Feed Health Report\n\n");

    out.push_str(
        "> **Handling Notice:** This report may contain sensitive threat intelligence, internal triage notes, indicators, and source metadata. Share only with appropriate recipients.\n\n",
    );

    out.push_str("## Summary\n\n");
    out.push_str(&format!("**Total Feeds:** {}\n\n", report.total_feeds));

    out.push_str("| Status | Count |\n");
    out.push_str("|---|---|\n");
    out.push_str(&format!("| Healthy | {} |\n", report.healthy_feeds));
    out.push_str(&format!("| Warning | {} |\n", report.warning_feeds));
    out.push_str(&format!("| Error | {} |\n", report.error_feeds));
    out.push_str(&format!("| Disabled | {} |\n", report.disabled_feeds));
    out.push('\n');

    if !report.feeds.is_empty() {
        out.push_str("## Feed Health Details\n\n");
        out.push_str(
            "| Feed | Type | Status | Failures | Last Fetch | Last Success | Last Error |\n",
        );
        out.push_str("|---|---|---|---|---|---|---|\n");
        for feed in &report.feeds {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                escape_table_cell(&feed.name),
                escape_table_cell(&feed.feed_type),
                escape_table_cell(&feed.status),
                feed.consecutive_failures,
                escape_table_cell(feed.last_fetch_at.as_deref().unwrap_or("-")),
                escape_table_cell(feed.last_success_at.as_deref().unwrap_or("-")),
                escape_table_cell(feed.last_error.as_deref().unwrap_or("-")),
            ));
        }
        out.push('\n');
    }

    out
}

pub fn render_daily_summary_report(report: &DailySummaryReport) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "# ThreatDeck Daily Summary Report: {}\n\n",
        report.date
    ));

    out.push_str(
        "> **Handling Notice:** This report may contain sensitive threat intelligence, internal triage notes, indicators, and source metadata. Share only with appropriate recipients.\n\n",
    );

    out.push_str("## Executive Summary\n\n");
    out.push_str(&format!("- **Total Alerts:** {}\n", report.alert_count));
    out.push_str(&format!("- **Unread Alerts:** {}\n", report.unread_count));
    out.push_str(&format!(
        "- **Critical Alerts:** {}\n",
        report.critical_count
    ));
    out.push_str(&format!("- **High Alerts:** {}\n", report.high_count));
    out.push_str(&format!("- **Feeds Checked:** {}\n", report.feeds_checked));
    out.push_str(&format!("- **Failed Feeds:** {}\n\n", report.failed_feeds));

    if !report.top_keywords.is_empty() {
        out.push_str("## Top Keywords\n\n");
        out.push_str("| Keyword | Count |\n");
        out.push_str("|---|---|\n");
        for kw in &report.top_keywords {
            out.push_str(&format!(
                "| {} | {} |\n",
                escape_table_cell(&kw.label),
                kw.count
            ));
        }
        out.push('\n');
    }

    if !report.top_feeds.is_empty() {
        out.push_str("## Top Feeds\n\n");
        out.push_str("| Feed | Count |\n");
        out.push_str("|---|---|\n");
        for feed in &report.top_feeds {
            out.push_str(&format!(
                "| {} | {} |\n",
                escape_table_cell(&feed.label),
                feed.count
            ));
        }
        out.push('\n');
    }

    if !report.alerts.is_empty() {
        out.push_str("## Critical Alerts\n\n");
        out.push_str("| ID | Title | Criticality | Status | Detected | Feed | Keyword |\n");
        out.push_str("|---|---|---|---|---|---|---|\n");
        for alert in &report.alerts {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                alert.alert_id,
                escape_table_cell(&alert.title),
                escape_table_cell(&alert.criticality),
                escape_table_cell(&alert.status),
                escape_table_cell(&alert.detected_at),
                escape_table_cell(&alert.feed_name),
                escape_table_cell(&alert.keyword_pattern),
            ));
        }
        out.push('\n');
    }

    out
}

pub fn escape_table_cell(input: &str) -> String {
    input
        .replace('|', "\\|")
        .replace('\n', " ")
        .replace('\r', "")
}

pub fn fenced_code_block(language: Option<&str>, input: &str) -> String {
    let lang = language.unwrap_or("");
    format!("```{lang}\n{input}\n```\n")
}

pub fn redact_sensitive(input: &str) -> String {
    let patterns = [
        (
            regex::Regex::new(r"(?i)(api[_-]?key\s*[:=]\s*)[^\s&]+").unwrap(),
            "${1}***REDACTED***",
        ),
        (
            regex::Regex::new(r"(?i)(authorization\s*[:=]\s*bearer\s+)\S+").unwrap(),
            "${1}***REDACTED***",
        ),
        (
            regex::Regex::new(r"(?i)(token\s*[:=]\s*)[^\s&]+").unwrap(),
            "${1}***REDACTED***",
        ),
        (
            regex::Regex::new(r"(?i)(password\s*[:=]\s*)[^\s&]+").unwrap(),
            "${1}***REDACTED***",
        ),
        (
            regex::Regex::new(r"(?i)(secret\s*[:=]\s*)[^\s&]+").unwrap(),
            "${1}***REDACTED***",
        ),
    ];

    let mut result = input.to_string();
    for (pattern, replacement) in &patterns {
        result = pattern.replace_all(&result, *replacement).to_string();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_table_cell() {
        assert_eq!(escape_table_cell("hello|world"), "hello\\|world");
        assert_eq!(escape_table_cell("hello\nworld"), "hello world");
        assert_eq!(escape_table_cell("hello\r\nworld"), "hello world");
    }

    #[test]
    fn test_fenced_code_block() {
        let block = fenced_code_block(Some("json"), "{\"key\": \"value\"}");
        assert!(block.starts_with("```json\n"));
        assert!(block.ends_with("\n```\n"));
    }

    #[test]
    fn test_redact_sensitive() {
        let input = "api_key=secret123\nAuthorization: Bearer token456";
        let redacted = redact_sensitive(input);
        assert!(!redacted.contains("secret123"));
        assert!(!redacted.contains("token456"));
        assert!(redacted.contains("***REDACTED***"));
    }
}

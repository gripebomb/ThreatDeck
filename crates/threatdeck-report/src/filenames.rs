use crate::types::ReportType;
use chrono::Utc;
use std::path::PathBuf;

pub fn generate_filename(
    report_type: ReportType,
    alert_id: Option<i64>,
    indicator_value: Option<&str>,
) -> String {
    let timestamp = Utc::now().format("%Y-%m-%d-%H%M%S");

    match report_type {
        ReportType::Alert => {
            let id = alert_id.unwrap_or(0);
            format!("alert-{id}-{timestamp}.md")
        }
        ReportType::AlertCollection => {
            format!("alerts-collection-{timestamp}.md")
        }
        ReportType::Case => {
            let id = alert_id.unwrap_or(0);
            format!("case-{id}-{timestamp}.md")
        }
        ReportType::Indicator => {
            let value = indicator_value.unwrap_or("unknown");
            let safe_value = sanitize_filename(value);
            format!("indicator-{safe_value}-{timestamp}.md")
        }
        ReportType::FeedHealth => {
            format!("feed-health-{timestamp}.md")
        }
        ReportType::DailySummary => {
            format!("daily-summary-{timestamp}.md")
        }
    }
}

pub fn sanitize_filename(input: &str) -> String {
    let mut result = input.to_lowercase();
    result = result.replace(' ', "-");
    result = result.replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "");
    result = result.replace("--", "-");
    if result.len() > 64 {
        result.truncate(64);
    }
    result.trim_matches('-').to_string()
}

pub fn ensure_safe_path(base_dir: &std::path::Path, filename: &str) -> Option<PathBuf> {
    let path = base_dir.join(filename);
    let canonical_base = std::fs::canonicalize(base_dir).ok()?;
    let canonical_path = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());

    if canonical_path.starts_with(&canonical_base) {
        Some(path)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_filename_alert() {
        let name = generate_filename(ReportType::Alert, Some(123), None);
        assert!(name.starts_with("alert-123-"));
        assert!(name.ends_with(".md"));
    }

    #[test]
    fn test_generate_filename_feed_health() {
        let name = generate_filename(ReportType::FeedHealth, None, None);
        assert!(name.starts_with("feed-health-"));
        assert!(name.ends_with(".md"));
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("example.com"), "examplecom");
        assert_eq!(sanitize_filename("hello world"), "hello-world");
        assert_eq!(sanitize_filename("test--value"), "test-value");
    }
}

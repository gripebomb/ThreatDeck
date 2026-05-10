use crate::app::App;
use crate::ui::list::{motion_from_key, move_selection, selected_style};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let type_filter = app
        .indicators_filter_type
        .map(indicator_type_label)
        .unwrap_or("all");
    let title_text = if app.filter_active {
        format!("Indicators | / {}", app.indicators_filter)
    } else {
        format!(
            "Indicators | Type: {} | Filter: {}",
            type_filter,
            if app.indicators_filter.is_empty() {
                "none"
            } else {
                &app.indicators_filter
            }
        )
    };
    let title = Paragraph::new(title_text)
        .style(
            Style::default()
                .fg(app.theme.primary)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(app.theme.border)),
        );
    f.render_widget(title, chunks[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
        .split(chunks[1]);
    draw_indicator_table(f, app, body[0]);
    draw_indicator_detail(f, app, body[1]);

    let status_text = if app.filter_active {
        "-- FILTER -- Type search | [Enter] Keep | [Esc] Clear"
    } else {
        "-- NORMAL -- [1-9,0] Nav  [e] Enrich  [t] Type  [c] Clear type  [r] Refresh  [/] Filter  [?] Help  [q] Quit"
    };
    let status = Paragraph::new(status_text).style(Style::default().fg(app.theme.muted));
    f.render_widget(status, chunks[2]);
}

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if let Some(motion) = motion_from_key(key, &mut app.pending_g) {
        app.indicators_selected =
            move_selection(app.indicators_selected, app.indicators_list.len(), motion);
        return;
    }

    match key.code {
        KeyCode::Char('t') => {
            app.indicators_filter_type = next_type_filter(app.indicators_filter_type);
            app.refresh_indicators();
        }
        KeyCode::Char('c') => {
            app.indicators_filter_type = None;
            app.refresh_indicators();
        }
        KeyCode::Char('e') => queue_selected_indicator_enrichment(app),
        KeyCode::Char('r') => app.refresh_indicators(),
        KeyCode::Char('/') => {
            app.filter_active = true;
            app.input_mode = crate::app::InputMode::Typing;
        }
        _ => {}
    }
}

fn queue_selected_indicator_enrichment(app: &mut App) {
    if !app.config.enrichment.enabled {
        app.set_notification(
            "Enrichment queueing is disabled in settings".to_string(),
            crate::types::NotificationType::Warning,
        );
        return;
    }

    let Some(indicator) = app.indicators_list.get(app.indicators_selected).cloned() else {
        return;
    };

    match app.db.queue_enrichment_jobs_for_indicators(&[indicator.id]) {
        Ok(job_ids) if job_ids.is_empty() => {
            app.set_notification(
                format!(
                    "No enrichment jobs queued for {}",
                    indicator.normalized_value
                ),
                crate::types::NotificationType::Info,
            );
        }
        Ok(job_ids) => {
            app.refresh_enrichment_queue();
            app.set_notification(
                format!(
                    "Queued {} enrichment job(s) for {}",
                    job_ids.len(),
                    indicator.normalized_value
                ),
                crate::types::NotificationType::Success,
            );
        }
        Err(e) => {
            app.set_notification(
                format!("Unable to queue enrichment: {}", e),
                crate::types::NotificationType::Error,
            );
        }
    }
}

fn draw_indicator_table(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let header = Row::new(vec![
        "Type",
        "Value",
        "Reputation",
        "Sightings",
        "Last Seen",
    ])
    .style(
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(app.theme.primary),
    );
    let mut table_state = TableState::default();
    table_state.select(Some(app.indicators_selected));
    let rows = app.indicators_list.iter().map(|indicator| {
        Row::new(vec![
            Cell::from(indicator_type_label(indicator.indicator_type)),
            Cell::from(truncate_chars(&indicator.normalized_value, 64)),
            Cell::from(enrichment_reputation_label(
                &app.db
                    .get_latest_enrichment_results(indicator.id)
                    .unwrap_or_default(),
            )),
            Cell::from(indicator.sighting_count.to_string()),
            Cell::from(indicator.last_seen_at.format("%Y-%m-%d %H:%M").to_string()),
        ])
        .style(Style::default().fg(app.theme.fg))
    });

    let table = Table::new(
        rows,
        vec![
            Constraint::Length(12),
            Constraint::Min(28),
            Constraint::Length(14),
            Constraint::Length(9),
            Constraint::Length(16),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border)),
    )
    .row_highlight_style(selected_style());
    f.render_stateful_widget(table, area, &mut table_state);
}

fn draw_indicator_detail(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let block = Block::default()
        .title("Detail")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.border));
    let Some(indicator) = app.indicators_list.get(app.indicators_selected) else {
        let empty = Paragraph::new("No indicators.")
            .block(block)
            .style(Style::default().fg(app.theme.muted));
        f.render_widget(empty, area);
        return;
    };

    let detail = app
        .db
        .get_indicator_detail(indicator.id)
        .unwrap_or_default();
    let enrichment_results = app
        .db
        .get_latest_enrichment_results(indicator.id)
        .unwrap_or_default();
    let enrichment_summary = enrichment_results
        .first()
        .and_then(|result| result.summary.as_deref())
        .unwrap_or("-");
    let mut lines = vec![
        format!("Type: {}", indicator_type_label(indicator.indicator_type)),
        format!("Value: {}", indicator.normalized_value),
        format!("Sightings: {}", indicator.sighting_count),
        format!(
            "Confidence: {}",
            indicator
                .confidence_score
                .map(|score| score.to_string())
                .unwrap_or_else(|| "-".into())
        ),
        format!(
            "Risk: {}",
            indicator
                .risk_score
                .map(|score| score.to_string())
                .unwrap_or_else(|| "-".into())
        ),
        format!(
            "First Seen: {}",
            indicator.first_seen_at.format("%Y-%m-%d %H:%M")
        ),
        format!(
            "Last Seen: {}",
            indicator.last_seen_at.format("%Y-%m-%d %H:%M")
        ),
        format!(
            "Reputation: {}",
            enrichment_reputation_label(&enrichment_results)
        ),
        format!("Enrichment: {}", enrichment_summary),
        String::new(),
        "Recent Occurrences:".to_string(),
    ];

    match detail {
        Some(detail) if !detail.occurrences.is_empty() => {
            for occurrence in detail.occurrences.iter().take(6) {
                lines.push(format!(
                    "- {} | {} | {}",
                    occurrence_location_label(occurrence),
                    occurrence
                        .source_field
                        .as_deref()
                        .unwrap_or("unknown field"),
                    occurrence.detected_at.format("%Y-%m-%d %H:%M")
                ));
                if let Some(surrounding_text) = occurrence.surrounding_text.as_deref() {
                    lines.push(format!("  {}", truncate_chars(surrounding_text, 72)));
                }
            }
        }
        _ => lines.push("- No occurrence records found.".to_string()),
    }

    let lines = lines.join("\n");
    let detail = Paragraph::new(lines)
        .block(block)
        .style(Style::default().fg(app.theme.fg).bg(app.theme.surface))
        .wrap(ratatui::widgets::Wrap { trim: false });
    f.render_widget(detail, area);
}

fn next_type_filter(
    current: Option<sentinel_ioc::IndicatorType>,
) -> Option<sentinel_ioc::IndicatorType> {
    use sentinel_ioc::IndicatorType;
    match current {
        None => Some(IndicatorType::Ipv4),
        Some(IndicatorType::Ipv4) => Some(IndicatorType::Ipv6),
        Some(IndicatorType::Ipv6) => Some(IndicatorType::Domain),
        Some(IndicatorType::Domain) => Some(IndicatorType::Url),
        Some(IndicatorType::Url) => Some(IndicatorType::Email),
        Some(IndicatorType::Email) => Some(IndicatorType::Md5),
        Some(IndicatorType::Md5) => Some(IndicatorType::Sha1),
        Some(IndicatorType::Sha1) => Some(IndicatorType::Sha256),
        Some(IndicatorType::Sha256) => Some(IndicatorType::Cve),
        Some(IndicatorType::Cve) => Some(IndicatorType::MitreAttackTechnique),
        Some(IndicatorType::MitreAttackTechnique) => Some(IndicatorType::OnionDomain),
        Some(IndicatorType::OnionDomain) => Some(IndicatorType::OnionUrl),
        Some(IndicatorType::OnionUrl) => None,
        Some(_) => None,
    }
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

pub(crate) fn enrichment_reputation_label(results: &[crate::db::EnrichmentResultRecord]) -> String {
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

fn occurrence_location_label(occurrence: &crate::db::IndicatorOccurrence) -> String {
    if let Some(alert_id) = occurrence.alert_id {
        format!("Alert #{alert_id}")
    } else if let Some(content_item_id) = occurrence.content_item_id {
        format!("Content #{content_item_id}")
    } else if let Some(feed_id) = occurrence.feed_id {
        format!("Feed #{feed_id}")
    } else {
        "Unlinked".to_string()
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}...", truncated)
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{enrichment_reputation_label, occurrence_location_label};
    use crate::db::{EnrichmentResultRecord, IndicatorOccurrence};
    use chrono::Utc;

    fn result(reputation: Option<&str>, verdict: Option<&str>) -> EnrichmentResultRecord {
        EnrichmentResultRecord {
            id: 1,
            indicator_id: 2,
            provider_id: 3,
            status: "succeeded".into(),
            reputation: reputation.map(str::to_string),
            score: None,
            verdict: verdict.map(str::to_string),
            summary: None,
            raw_json: None,
            fetched_at: Utc::now(),
            expires_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn enrichment_reputation_prefers_verdict_then_reputation() {
        assert_eq!(
            enrichment_reputation_label(&[result(Some("Malicious"), Some("Known Exploited"))]),
            "Known Exploited"
        );
        assert_eq!(
            enrichment_reputation_label(&[result(Some("Suspicious"), None)]),
            "Suspicious"
        );
        assert_eq!(enrichment_reputation_label(&[]), "Unknown");
    }

    #[test]
    fn occurrence_location_prefers_alert_then_content_then_feed() {
        let base = IndicatorOccurrence {
            id: 1,
            indicator_id: 2,
            content_item_id: Some(3),
            alert_id: Some(4),
            feed_id: Some(5),
            source_field: None,
            start_offset: None,
            end_offset: None,
            surrounding_text: None,
            detected_at: Utc::now(),
        };
        assert_eq!(occurrence_location_label(&base), "Alert #4");

        let mut content = base.clone();
        content.alert_id = None;
        assert_eq!(occurrence_location_label(&content), "Content #3");

        let mut feed = content.clone();
        feed.content_item_id = None;
        assert_eq!(occurrence_location_label(&feed), "Feed #5");
    }
}

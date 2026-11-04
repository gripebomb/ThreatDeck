pub mod alert_details;
pub mod alert_list;
pub mod context_tabs;
pub mod events;
pub mod layout;
pub mod render;
pub mod state;

use crate::app::App;
use crate::ui::list::{criticality_label, motion_from_key, move_selection};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
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

    let title_text = if app.filter_active {
        format!("Alerts | / {}", app.alerts_filter)
    } else {
        let crit = app
            .alerts_filter_criticality
            .map(|c| format!("{:?}", c))
            .unwrap_or_else(|| "all".into());
        let status = app
            .alerts_filter_status
            .map(|s| format!("{:?}", s))
            .unwrap_or_else(|| "all".into());
        let disp = app
            .alerts_filter_disposition
            .map(|d| format!("{:?}", d))
            .unwrap_or_else(|| "all".into());
        format!(
            "Alerts | Filter: {} | Crit: {} | Status: {} | Disp: {}{}{}",
            if app.alerts_filter.is_empty() {
                "none"
            } else {
                &app.alerts_filter
            },
            crit,
            status,
            disp,
            if app.alerts_hide_closed {
                " | HideClosed"
            } else {
                ""
            },
            if app.alerts_bulk_mode { " | BULK" } else { "" }
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

    render::draw_workbench(f, app, chunks[1]);

    let status_text = if app.filter_active {
        "-- FILTER -- Type search | [Enter] Keep | [Esc] Clear"
    } else if app.alerts_bulk_mode {
        "-- BULK -- [Space] Select  [a] All  [d] Delete selected  [Esc] Cancel"
    } else if app.triage_enum_select_mode {
        "-- SELECT -- [1-9] Pick option  [0/Esc] Cancel"
    } else if app.triage_note_input_mode {
        match app.triage_input_target {
            crate::types::TriageInputTarget::Note => {
                "-- NOTE -- Type note | [Enter] Save | [Esc] Cancel"
            }
            crate::types::TriageInputTarget::Owner => {
                "-- OWNER -- Type analyst name | [Enter] Save | [Esc] Cancel"
            }
        }
    } else {
        "-- NORMAL -- [r] Read  [A] Acknowledge  [I] Investigate  [E] Escalate  [C] Close  [O] Reopen  [N] Note  [M] Owner  [Enter] Detail  [d] Delete  [/] Filter  [?] Help  [q] Quit"
    };
    let status = Paragraph::new(status_text).style(Style::default().fg(app.theme.muted));
    f.render_widget(status, chunks[2]);

    if app.alerts_detail_view {
        draw_detail(f, app);
    }

    if app.triage_enum_select_mode {
        draw_enum_selector(f, app);
    }

    if app.triage_note_input_mode {
        draw_note_input(f, app);
    }

    if app.triage_history_view {
        draw_history(f, app);
    }
}

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if app.triage_enum_select_mode {
        handle_enum_select_key(app, key);
        return;
    }
    if app.triage_note_input_mode {
        handle_note_input_key(app, key);
        return;
    }
    if app.triage_history_view {
        if key.code == KeyCode::Esc {
            app.triage_history_view = false;
        }
        return;
    }
    if app.alerts_bulk_mode {
        handle_bulk_key(app, key);
        return;
    }
    events::handle_workbench_key(app, key);
}

fn handle_bulk_key(app: &mut App, key: KeyEvent) {
    if let Some(motion) = motion_from_key(key, &mut app.pending_g) {
        app.alerts_selected = move_selection(app.alerts_selected, app.alerts_list.len(), motion);
        return;
    }

    match key.code {
        KeyCode::Char(' ') => {
            if let Some(a) = app.alerts_list.get(app.alerts_selected) {
                let id = a.alert.id;
                if app.alerts_selected_bulk.contains(&id) {
                    app.alerts_selected_bulk.remove(&id);
                } else {
                    app.alerts_selected_bulk.insert(id);
                }
            }
        }
        KeyCode::Char('a') => {
            for a in &app.alerts_list {
                app.alerts_selected_bulk.insert(a.alert.id);
            }
        }
        KeyCode::Char('d') if !app.alerts_selected_bulk.is_empty() => {
            app.show_confirm = Some(crate::types::ConfirmDialog::BulkDeleteAlerts {
                count: app.alerts_selected_bulk.len(),
            });
        }
        KeyCode::Esc => {
            app.alerts_bulk_mode = false;
            app.alerts_selected_bulk.clear();
        }
        _ => {}
    }
}

fn draw_detail(f: &mut Frame, app: &App) {
    let area = f.area();
    let detail_area = ratatui::layout::Rect {
        x: area.width / 8,
        y: area.height / 8,
        width: area.width * 3 / 4,
        height: area.height * 3 / 4,
    };
    f.render_widget(Clear, detail_area);
    let Some(alert) = app.alerts_list.get(app.alerts_selected) else {
        return;
    };
    let block = Block::default()
        .title("Alert Detail - Esc to close")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.primary));
    let inner = block.inner(detail_area);
    f.render_widget(block, detail_area);
    let detail_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),
            Constraint::Length(10),
            Constraint::Min(6),
        ])
        .split(inner);
    let meta_lines = [
        format!(
            "Title: {}",
            alert.alert.title.as_deref().unwrap_or("(untitled)")
        ),
        format!(
            "Criticality: {} (Effective: {})",
            criticality_label(alert.alert.criticality),
            criticality_label(alert.alert.effective_severity())
        ),
        format!("Feed: {}", alert.feed_name),
        format!("Keyword: {}", alert.keyword_pattern),
        format!(
            "Detected: {}",
            alert.alert.detected_at.format("%Y-%m-%d %H:%M:%S")
        ),
        format!("Read: {}", if alert.alert.read { "yes" } else { "no" }),
        format!(
            "Tags: {}",
            alert
                .tags
                .iter()
                .map(|t| t.name.clone())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        String::new(),
        alert.alert.content_snippet.clone(),
    ]
    .join("\n");
    let meta_para = Paragraph::new(meta_lines)
        .style(Style::default().fg(app.theme.fg).bg(app.theme.surface))
        .wrap(ratatui::widgets::Wrap { trim: false });
    f.render_widget(meta_para, detail_chunks[0]);
    draw_triage_panel(f, app, detail_chunks[1], alert);
    draw_indicator_panel(f, app, detail_chunks[2], alert.alert.id);
}

fn draw_triage_panel(
    f: &mut Frame,
    app: &App,
    area: ratatui::layout::Rect,
    alert: &crate::types::AlertWithMeta,
) {
    let block = Block::default()
        .title("Triage")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.border));
    let mut lines = vec![];
    lines.push(format!("Status:        {}", alert.alert.status));
    lines.push(format!("Disposition:   {}", alert.alert.disposition));
    lines.push(format!(
        "Severity:      {}",
        criticality_label(alert.alert.effective_severity())
    ));
    lines.push(format!(
        "Confidence:    {}",
        alert
            .alert
            .confidence_score
            .map(|c| c.to_string())
            .unwrap_or_else(|| "-".into())
    ));
    lines.push(format!(
        "Owner:         {}",
        alert.alert.owner.as_deref().unwrap_or("-")
    ));
    lines.push(String::new());
    lines.push(format!(
        "Notes: {}",
        alert.alert.triage_notes.as_deref().unwrap_or("(none)")
    ));
    lines.push(String::new());
    if let Some(ts) = alert.alert.acknowledged_at {
        lines.push(format!("Acknowledged:  {}", ts.format("%Y-%m-%d %H:%M")));
    }
    if let Some(ts) = alert.alert.investigating_at {
        lines.push(format!("Investigating: {}", ts.format("%Y-%m-%d %H:%M")));
    }
    if let Some(ts) = alert.alert.escalated_at {
        lines.push(format!("Escalated:     {}", ts.format("%Y-%m-%d %H:%M")));
    }
    if let Some(ts) = alert.alert.closed_at {
        lines.push(format!(
            "Closed:        {} — {}",
            ts.format("%Y-%m-%d %H:%M"),
            alert
                .alert
                .closed_reason
                .as_deref()
                .unwrap_or("(no reason)")
        ));
    }
    let para = Paragraph::new(lines.join("\n"))
        .block(block)
        .style(Style::default().fg(app.theme.fg).bg(app.theme.surface));
    f.render_widget(para, area);
}

fn draw_indicator_panel(f: &mut Frame, app: &App, area: ratatui::layout::Rect, alert_id: i64) {
    let block = Block::default()
        .title("Indicators")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.border));
    let indicators = app
        .db
        .list_indicators_for_alert(alert_id)
        .unwrap_or_default();
    if indicators.is_empty() {
        let empty = Paragraph::new("No indicators extracted for this alert.")
            .block(block)
            .style(Style::default().fg(app.theme.muted).bg(app.theme.surface));
        f.render_widget(empty, area);
        return;
    }

    let header = Row::new(vec!["Type", "Value", "Sightings", "Reputation"]).style(
        Style::default()
            .fg(app.theme.primary)
            .add_modifier(Modifier::BOLD),
    );
    let rows = indicators.iter().take(6).map(|indicator| {
        let enrichment_results = app
            .db
            .get_latest_enrichment_results(indicator.id)
            .unwrap_or_default();
        Row::new(vec![
            Cell::from(indicator_type_label(indicator.indicator_type)),
            Cell::from(truncate_chars(&indicator.normalized_value, 56)),
            Cell::from(indicator.sighting_count.to_string()),
            Cell::from(crate::ui::indicators::enrichment_reputation_label(
                &enrichment_results,
            )),
        ])
        .style(Style::default().fg(app.theme.fg))
    });
    let table = Table::new(
        rows,
        vec![
            Constraint::Length(12),
            Constraint::Min(24),
            Constraint::Length(9),
            Constraint::Length(12),
        ],
    )
    .header(header)
    .block(block);
    f.render_widget(table, area);
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

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}...", truncated)
    } else {
        value.to_string()
    }
}

fn draw_enum_selector(f: &mut Frame, app: &App) {
    let area = f.area();
    let popup = ratatui::layout::Rect {
        x: area.width / 3,
        y: area.height / 3,
        width: area.width / 3,
        height: 12,
    };
    f.render_widget(Clear, popup);

    let target = app
        .triage_enum_target
        .unwrap_or(crate::types::TriageEnumTarget::Status);
    let title = match target {
        crate::types::TriageEnumTarget::Status => "Select Status",
        crate::types::TriageEnumTarget::Disposition => "Select Disposition",
        crate::types::TriageEnumTarget::Severity => "Select Severity",
    };

    let items: Vec<&str> = match target {
        crate::types::TriageEnumTarget::Status => {
            vec![
                "New",
                "Acknowledged",
                "Investigating",
                "Escalated",
                "Closed",
            ]
        }
        crate::types::TriageEnumTarget::Disposition => {
            vec![
                "Unknown",
                "ConfirmedThreat",
                "FalsePositive",
                "Benign",
                "Duplicate",
                "Informational",
                "NeedsMoreContext",
            ]
        }
        crate::types::TriageEnumTarget::Severity => {
            vec!["Low", "Medium", "High", "Critical"]
        }
    };

    let list_items: Vec<ratatui::widgets::ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, &item)| {
            let num = i + 1;
            ratatui::widgets::ListItem::new(format!("{num}. {item}"))
        })
        .collect();
    let list = ratatui::widgets::List::new(list_items)
        .block(
            Block::default()
                .title(format!("{title} — press number key"))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.theme.primary)),
        )
        .style(Style::default().fg(app.theme.fg).bg(app.theme.surface));
    f.render_widget(list, popup);
}

fn handle_enum_select_key(app: &mut App, key: KeyEvent) {
    let target = app
        .triage_enum_target
        .unwrap_or(crate::types::TriageEnumTarget::Status);

    match key.code {
        KeyCode::Esc => {
            app.triage_enum_select_mode = false;
            app.triage_enum_target = None;
            app.triage_enum_selected = 0;
        }
        KeyCode::Char(c) if c.is_ascii_digit() => {
            let digit = c as usize - '0' as usize;
            if digit == 0 {
                app.triage_enum_select_mode = false;
                app.triage_enum_target = None;
                app.triage_enum_selected = 0;
                return;
            }
            let idx = digit.saturating_sub(1);
            if let Some(a) = app.alerts_list.get(app.alerts_selected) {
                let alert_id = a.alert.id;
                match target {
                    crate::types::TriageEnumTarget::Status => {
                        let statuses = [
                            crate::types::AlertStatus::New,
                            crate::types::AlertStatus::Acknowledged,
                            crate::types::AlertStatus::Investigating,
                            crate::types::AlertStatus::Escalated,
                            crate::types::AlertStatus::Closed,
                        ];
                        if let Some(&status) = statuses.get(idx) {
                            let note = Some(&*format!("Set to {:?}", status));
                            let result = app
                                .db
                                .update_alert_status(alert_id, status, note)
                                .and_then(|()| app.db.mark_alert_read(alert_id, true));
                            app.finish_triage_mutation(
                                result,
                                format!("Status: {:?}", status),
                                crate::types::NotificationType::Success,
                            );
                        }
                    }
                    crate::types::TriageEnumTarget::Disposition => {
                        let dispositions = [
                            crate::types::AlertDisposition::Unknown,
                            crate::types::AlertDisposition::ConfirmedThreat,
                            crate::types::AlertDisposition::FalsePositive,
                            crate::types::AlertDisposition::Benign,
                            crate::types::AlertDisposition::Duplicate,
                            crate::types::AlertDisposition::Informational,
                            crate::types::AlertDisposition::NeedsMoreContext,
                        ];
                        if let Some(&disp) = dispositions.get(idx) {
                            let result = app.db.update_alert_disposition(alert_id, disp, None);
                            app.finish_triage_mutation(
                                result,
                                format!("Disposition: {:?}", disp),
                                crate::types::NotificationType::Success,
                            );
                        }
                    }
                    crate::types::TriageEnumTarget::Severity => {
                        let severities = [
                            crate::types::Criticality::Low,
                            crate::types::Criticality::Medium,
                            crate::types::Criticality::High,
                            crate::types::Criticality::Critical,
                        ];
                        if let Some(&sev) = severities.get(idx) {
                            let result = app.db.update_alert_severity(alert_id, Some(sev), None);
                            app.finish_triage_mutation(
                                result,
                                format!("Severity: {:?}", sev),
                                crate::types::NotificationType::Success,
                            );
                        }
                    }
                }
            }
            app.triage_enum_select_mode = false;
            app.triage_enum_target = None;
            app.triage_enum_selected = 0;
        }
        _ => {}
    }
}

fn draw_note_input(f: &mut Frame, app: &App) {
    let area = f.area();
    let popup = ratatui::layout::Rect {
        x: area.width / 4,
        y: area.height / 3,
        width: area.width / 2,
        height: 5,
    };
    f.render_widget(Clear, popup);
    let title = match app.triage_input_target {
        crate::types::TriageInputTarget::Note => "Add Note — Enter to save, Esc to cancel",
        crate::types::TriageInputTarget::Owner => "Assign Owner — Enter to save, Esc to cancel",
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.primary));
    let para = Paragraph::new(app.triage_note_input.as_str())
        .block(block)
        .style(Style::default().fg(app.theme.fg).bg(app.theme.surface));
    f.render_widget(para, popup);
}

fn handle_note_input_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.triage_note_input_mode = false;
            app.triage_note_input.clear();
            app.triage_input_target = crate::types::TriageInputTarget::default();
            app.input_mode = crate::app::InputMode::Normal;
        }
        KeyCode::Enter => {
            if let Some(a) = app.alerts_list.get(app.alerts_selected) {
                if !app.triage_note_input.is_empty() {
                    let (result, success_message) = match app.triage_input_target {
                        crate::types::TriageInputTarget::Note => (
                            app.db.add_alert_note(a.alert.id, &app.triage_note_input),
                            "Note saved — shown in Triage panel below".to_string(),
                        ),
                        crate::types::TriageInputTarget::Owner => (
                            app.db.assign_alert_owner(
                                a.alert.id,
                                Some(&app.triage_note_input),
                                None,
                            ),
                            format!("Owner set to {}", app.triage_note_input),
                        ),
                    };
                    app.finish_triage_mutation(
                        result,
                        success_message,
                        crate::types::NotificationType::Success,
                    );
                }
            }
            app.triage_note_input_mode = false;
            app.triage_note_input.clear();
            app.triage_input_target = crate::types::TriageInputTarget::default();
            app.input_mode = crate::app::InputMode::Normal;
            app.refresh_alerts();
            app.alerts_detail_view = true;
        }
        KeyCode::Backspace => {
            app.triage_note_input.pop();
        }
        KeyCode::Char(c) => {
            app.triage_note_input.push(c);
        }
        _ => {}
    }
}

fn draw_history(f: &mut Frame, app: &App) {
    let area = f.area();
    let popup = ratatui::layout::Rect {
        x: area.width / 8,
        y: area.height / 6,
        width: area.width * 3 / 4,
        height: area.height * 2 / 3,
    };
    f.render_widget(Clear, popup);
    let block = Block::default()
        .title("Triage History — Esc to close")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.primary));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let Some(alert) = app.alerts_list.get(app.alerts_selected) else {
        return;
    };
    let events = app
        .db
        .list_alert_triage_events(alert.alert.id)
        .unwrap_or_default();
    let mut lines = vec![];
    if events.is_empty() {
        lines.push("No triage events recorded.".to_string());
    } else {
        for event in events {
            let ts = event.created_at.format("%Y-%m-%d %H:%M");
            let change = match (event.old_value.as_ref(), event.new_value.as_ref()) {
                (Some(old), Some(new)) => format!("{} → {}", old, new),
                (None, Some(new)) => format!("→ {}", new),
                (Some(old), None) => format!("{} → (none)", old),
                (None, None) => String::new(),
            };
            let note = event.note.map(|n| format!(" — {}", n)).unwrap_or_default();
            lines.push(format!("{} {:20} {}{}", ts, event.event_type, change, note));
        }
    }
    let para = Paragraph::new(lines.join("\n"))
        .style(Style::default().fg(app.theme.fg).bg(app.theme.surface));
    f.render_widget(para, inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, Paths};
    use crate::db::{AlertCreate, Db, FeedCreate, KeywordCreate};
    use crate::types::{Criticality, FeedType};
    use ratatui::{backend::TestBackend, Terminal};
    use std::path::{Path, PathBuf};

    fn temp_db_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "threatdeck-alerts-ui-{}-{}.db",
            name,
            std::process::id()
        ))
    }

    #[test]
    fn draw_alerts_handles_multibyte_snippet_truncation() {
        let path = temp_db_path("multibyte");
        let _ = std::fs::remove_file(&path);
        let db = Db::open(&path).unwrap();
        db.init_schema().unwrap();
        let feed_id = db
            .create_feed(&FeedCreate {
                name: "Example".into(),
                url: "https://example.com/feed.xml".into(),
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
        let snippet = format!("{}é and more alert text", "a".repeat(39));
        let alert_id = db
            .create_alert(&AlertCreate {
                feed_id,
                keyword_id,
                title: Some("Unicode alert".into()),
                content_snippet: snippet,
                criticality: Criticality::High,
                content_hash: "unicode-alert-hash".into(),
                metadata_json: None,
            })
            .unwrap();
        db.store_extracted_indicators(
            &[sentinel_ioc::ExtractedIndicator {
                indicator_type: sentinel_ioc::IndicatorType::Cve,
                value: "CVE-2025-12345".into(),
                normalized_value: "CVE-2025-12345".into(),
                source_field: "content_snippet".into(),
                start_offset: 0,
                end_offset: 14,
                surrounding_text: "CVE-2025-12345".into(),
                confidence_hint: Some(90),
            }],
            Some(alert_id),
            None,
            Some(feed_id),
        )
        .unwrap();

        let paths = Paths {
            config_dir: PathBuf::new(),
            data_dir: PathBuf::new(),
            config_file: PathBuf::new(),
            db_file: path.clone(),
        };
        let mut app = App::new(db, AppConfig::default(), paths);
        app.screen = crate::types::Screen::Alerts;
        app.alerts_detail_view = true;
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn truncate_chars_never_splits_multibyte_characters() {
        assert_eq!(
            truncate_chars(&format!("{}é and more", "a".repeat(39)), 40),
            format!("{}é...", "a".repeat(39))
        );
    }

    fn setup_app_with_alerts(count: usize, path: &Path) -> App {
        let db = Db::open(path).unwrap();
        db.init_schema().unwrap();
        let feed_id = db
            .create_feed(&FeedCreate {
                name: "TestFeed".into(),
                url: "https://example.com/feed.xml".into(),
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
        for i in 0..count {
            db.create_alert(&AlertCreate {
                feed_id,
                keyword_id,
                title: Some(format!("Alert #{}", i)),
                content_snippet: format!("Suspicious activity detected #{}", i),
                criticality: Criticality::High,
                content_hash: format!("hash-{}", i),
                metadata_json: Some(
                    serde_json::json!({"source": "test", "priority": i}).to_string(),
                ),
            })
            .unwrap();
        }
        let paths = Paths {
            config_dir: PathBuf::new(),
            data_dir: PathBuf::new(),
            config_file: PathBuf::new(),
            db_file: path.to_path_buf(),
        };
        let mut app = App::new(db, AppConfig::default(), paths);
        app.screen = crate::types::Screen::Alerts;
        app
    }

    #[test]
    fn render_no_alerts() {
        let path = temp_db_path("empty");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app_with_alerts(0, &path);
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn render_one_alert() {
        let path = temp_db_path("one");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app_with_alerts(1, &path);
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn render_many_alerts() {
        let path = temp_db_path("many");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app_with_alerts(20, &path);
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn render_selected_alert_details() {
        let path = temp_db_path("details");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app_with_alerts(3, &path);
        // Bundle is auto-loaded by refresh_workbench in App::new
        assert!(app.workbench_bundle.is_some());
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn render_no_iocs() {
        let path = temp_db_path("noioc");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app_with_alerts(1, &path);
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        // IOCs tab should show empty state
        assert!(app.workbench_bundle.is_some());
        assert!(app.workbench_bundle.as_ref().unwrap().indicators.is_empty());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn render_metadata_tab() {
        let path = temp_db_path("meta");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app_with_alerts(2, &path);
        app.workbench_state.active_context_tab =
            crate::ui::alerts::state::AlertContextTab::Metadata;
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn render_triage_history_tab() {
        let path = temp_db_path("history");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app_with_alerts(1, &path);
        app.workbench_state.active_context_tab =
            crate::ui::alerts::state::AlertContextTab::TriageHistory;
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn render_narrow_terminal() {
        let path = temp_db_path("narrow");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app_with_alerts(5, &path);
        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn render_tiny_terminal() {
        let path = temp_db_path("tiny");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app_with_alerts(1, &path);
        let backend = TestBackend::new(10, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        // Must not panic
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let _ = std::fs::remove_file(path);
    }
}

use crate::app::App;
use crate::ui::list::{criticality_label, motion_from_key, move_selection, selected_style};
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
        format!(
            "Alerts | Filter: {} | Criticality: {}{}",
            if app.alerts_filter.is_empty() {
                "none"
            } else {
                &app.alerts_filter
            },
            crit,
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

    let header = Row::new(vec![
        "Read", "Crit", "Feed", "Keyword", "Snippet", "Detected", "Tags",
    ])
    .style(
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(app.theme.primary),
    );

    let rows: Vec<Row> = app
        .alerts_list
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let style = if i == app.alerts_selected {
                selected_style()
            } else {
                Style::default().fg(app.theme.fg)
            };
            let read_mark = if a.alert.read { "○" } else { "●" };
            let crit_color = crate::theme::criticality_color(app.theme, a.alert.criticality);
            let crit_str = criticality_label(a.alert.criticality);
            let snippet = truncate_chars(&a.alert.content_snippet, 40);
            let time_str = a.alert.detected_at.format("%Y-%m-%d %H:%M").to_string();
            let tag_str = a
                .tags
                .iter()
                .map(|t| t.name.clone())
                .collect::<Vec<_>>()
                .join(", ");

            Row::new(vec![
                Cell::from(read_mark).style(Style::default().fg(if a.alert.read {
                    app.theme.muted
                } else {
                    app.theme.primary
                })),
                Cell::from(crit_str)
                    .style(Style::default().fg(crit_color).add_modifier(Modifier::BOLD)),
                Cell::from(a.feed_name.as_str()),
                Cell::from(a.keyword_pattern.as_str()),
                Cell::from(snippet),
                Cell::from(time_str),
                Cell::from(tag_str),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        vec![
            Constraint::Length(5),
            Constraint::Length(10),
            Constraint::Min(15),
            Constraint::Min(12),
            Constraint::Min(25),
            Constraint::Length(16),
            Constraint::Min(12),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border)),
    );
    f.render_widget(table, chunks[1]);

    let status_text = if app.filter_active {
        "-- FILTER -- Type search | [Enter] Keep | [Esc] Clear"
    } else if app.alerts_bulk_mode {
        "-- BULK -- [Space] Select  [a] All  [d] Delete selected  [Esc] Cancel"
    } else {
        "-- NORMAL -- [1-8] Nav  [r] Read  [R] All read  [d] Delete  [D] Bulk  [c] Crit  [Enter] Detail  [/] Filter  [?] Help  [q] Quit"
    };
    let status = Paragraph::new(status_text).style(Style::default().fg(app.theme.muted));
    f.render_widget(status, chunks[2]);

    if app.alerts_detail_view {
        draw_detail(f, app);
    }
}

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if app.alerts_bulk_mode {
        handle_bulk_key(app, key);
        return;
    }

    if let Some(motion) = motion_from_key(key, &mut app.pending_g) {
        app.alerts_selected = move_selection(app.alerts_selected, app.alerts_list.len(), motion);
        return;
    }

    match key.code {
        KeyCode::Char('r') => {
            if let Some(a) = app.alerts_list.get(app.alerts_selected) {
                let new_read = !a.alert.read;
                let _ = app.db.mark_alert_read(a.alert.id, new_read);
                app.refresh_alerts();
                app.refresh_dashboard();
            }
        }
        KeyCode::Char('R') => {
            let _ = app.db.mark_all_alerts_read(true);
            app.refresh_alerts();
            app.refresh_dashboard();
            app.set_notification(
                "All alerts marked as read".to_string(),
                crate::types::NotificationType::Success,
            );
        }
        KeyCode::Char('d') => {
            if let Some(a) = app.alerts_list.get(app.alerts_selected) {
                app.show_confirm =
                    Some(crate::types::ConfirmDialog::DeleteAlert { id: a.alert.id });
            }
        }
        KeyCode::Char('D') => {
            app.alerts_bulk_mode = true;
            app.alerts_selected_bulk.clear();
        }
        KeyCode::Char('t') => {
            if let Some(a) = app.alerts_list.get(app.alerts_selected) {
                app.tags_assignment_mode = true;
                app.tags_assignment_target =
                    Some(crate::types::TagAssignmentTarget::Alert(a.alert.id));
                app.refresh_tags();
            }
        }
        KeyCode::Enter => app.alerts_detail_view = true,
        KeyCode::Char('/') => {
            app.filter_active = true;
            app.input_mode = crate::app::InputMode::Typing;
        }
        KeyCode::Char('c') => {
            app.alerts_filter_criticality = match app.alerts_filter_criticality {
                None => Some(crate::types::Criticality::Low),
                Some(crate::types::Criticality::Low) => Some(crate::types::Criticality::Medium),
                Some(crate::types::Criticality::Medium) => Some(crate::types::Criticality::High),
                Some(crate::types::Criticality::High) => Some(crate::types::Criticality::Critical),
                Some(crate::types::Criticality::Critical) => None,
            };
            app.refresh_alerts();
        }
        _ => {}
    }
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
        KeyCode::Char('d') => {
            if !app.alerts_selected_bulk.is_empty() {
                app.show_confirm = Some(crate::types::ConfirmDialog::BulkDeleteAlerts {
                    count: app.alerts_selected_bulk.len(),
                });
            }
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
        y: area.height / 6,
        width: area.width * 3 / 4,
        height: area.height * 2 / 3,
    };
    f.render_widget(Clear, detail_area);
    let Some(alert) = app.alerts_list.get(app.alerts_selected) else {
        return;
    };
    let block = Block::default()
        .title("Alert Detail - Esc to close")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.primary));
    let lines = vec![
        format!(
            "Title: {}",
            alert.alert.title.as_deref().unwrap_or("(untitled)")
        ),
        format!(
            "Criticality: {}",
            criticality_label(alert.alert.criticality)
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
        String::new(),
        format!(
            "Metadata: {}",
            alert.alert.metadata_json.as_deref().unwrap_or("{}")
        ),
    ]
    .join("\n");
    let para = Paragraph::new(lines)
        .block(block)
        .style(Style::default().fg(app.theme.fg).bg(app.theme.surface))
        .wrap(ratatui::widgets::Wrap { trim: false });
    f.render_widget(para, detail_area);
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
    use super::*;
    use crate::config::{AppConfig, Paths};
    use crate::db::{AlertCreate, Db, FeedCreate, KeywordCreate};
    use crate::types::{Criticality, FeedType};
    use ratatui::{backend::TestBackend, Terminal};
    use std::path::PathBuf;

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
        db.create_alert(&AlertCreate {
            feed_id,
            keyword_id,
            title: Some("Unicode alert".into()),
            content_snippet: snippet,
            criticality: Criticality::High,
            content_hash: "unicode-alert-hash".into(),
            metadata_json: None,
        })
        .unwrap();

        let paths = Paths {
            config_dir: PathBuf::new(),
            data_dir: PathBuf::new(),
            config_file: PathBuf::new(),
            db_file: path.clone(),
        };
        let mut app = App::new(db, AppConfig::default(), paths);
        app.screen = crate::types::Screen::Alerts;
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
}

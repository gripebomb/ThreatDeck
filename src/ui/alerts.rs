use ratatui::{Frame, layout::{Constraint, Direction, Layout}, style::{Style, Modifier}, widgets::{Block, Borders, Paragraph, Table, Row, Cell}};
use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent};

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    let title = Paragraph::new("Alerts")
        .style(Style::default().fg(app.theme.primary).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(app.theme.border)));
    f.render_widget(title, chunks[0]);

    let header = Row::new(vec!["Read", "Crit", "Feed", "Keyword", "Snippet", "Detected", "Tags"])
        .style(Style::default().add_modifier(Modifier::BOLD).fg(app.theme.primary));

    let rows: Vec<Row> = app.alerts_list.iter().enumerate().map(|(i, a)| {
        let style = if i == app.alerts_selected {
            Style::default().bg(app.theme.highlight).fg(app.theme.bg).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.fg)
        };
        let read_mark = if a.alert.read { "○" } else { "●" };
        let crit_color = crate::theme::criticality_color(app.theme, a.alert.criticality);
        let crit_str = format!("{:?}", a.alert.criticality);
        let snippet = if a.alert.content_snippet.len() > 40 {
            format!("{}...", &a.alert.content_snippet[..40])
        } else {
            a.alert.content_snippet.clone()
        };
        let time_str = a.alert.detected_at.format("%Y-%m-%d %H:%M").to_string();
        let tag_str = a.tags.iter().map(|t| t.name.clone()).collect::<Vec<_>>().join(", ");

        Row::new(vec![
            Cell::from(read_mark).style(Style::default().fg(if a.alert.read { app.theme.muted } else { app.theme.primary })),
            Cell::from(crit_str).style(Style::default().fg(crit_color).add_modifier(Modifier::BOLD)),
            Cell::from(a.feed_name.as_str()),
            Cell::from(a.keyword_pattern.as_str()),
            Cell::from(snippet),
            Cell::from(time_str),
            Cell::from(tag_str),
        ]).style(style)
    }).collect();

    let table = Table::new(rows, vec![
        Constraint::Length(5), Constraint::Length(10), Constraint::Min(15),
        Constraint::Min(12), Constraint::Min(25), Constraint::Length(16), Constraint::Min(12),
    ])
    .header(header)
    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(app.theme.border)));
    f.render_widget(table, chunks[1]);

    let status = Paragraph::new("[r] Toggle read  [R] Mark all read  [d] Delete  [D] Bulk mode  [t] Tags  [/] Filter  [q] Back")
        .style(Style::default().fg(app.theme.muted));
    f.render_widget(status, chunks[2]);
}

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if app.alerts_bulk_mode {
        handle_bulk_key(app, key);
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
            app.set_notification("All alerts marked as read".to_string(), crate::types::NotificationType::Success);
        }
        KeyCode::Char('d') => {
            if let Some(a) = app.alerts_list.get(app.alerts_selected) {
                app.show_confirm = Some(crate::types::ConfirmDialog::DeleteAlert { id: a.alert.id });
            }
        }
        KeyCode::Char('D') => {
            app.alerts_bulk_mode = true;
            app.alerts_selected_bulk.clear();
        }
        KeyCode::Char('t') => {
            app.set_notification("Tag assignment".to_string(), crate::types::NotificationType::Info);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if !app.alerts_list.is_empty() {
                app.alerts_selected = (app.alerts_selected + 1).min(app.alerts_list.len() - 1);
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.alerts_selected > 0 {
                app.alerts_selected -= 1;
            }
        }
        _ => {}
    }
}

fn handle_bulk_key(app: &mut App, key: KeyEvent) {
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
        KeyCode::Down | KeyCode::Char('j') => {
            if !app.alerts_list.is_empty() {
                app.alerts_selected = (app.alerts_selected + 1).min(app.alerts_list.len() - 1);
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.alerts_selected > 0 {
                app.alerts_selected -= 1;
            }
        }
        _ => {}
    }
}

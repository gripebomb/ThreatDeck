use ratatui::{Frame, layout::{Constraint, Direction, Layout}, style::{Style, Modifier}, widgets::{Block, Borders, Paragraph, Table, Row, Cell}};
use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent};

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    let title = Paragraph::new("Feed Health Logs")
        .style(Style::default().fg(app.theme.primary).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(app.theme.border)));
    f.render_widget(title, chunks[0]);

    let header = Row::new(vec!["Time", "Feed", "Status", "Error"])
        .style(Style::default().add_modifier(Modifier::BOLD).fg(app.theme.primary));

    let rows: Vec<Row> = app.logs_list.iter().enumerate().map(|(i, log)| {
        let style = if i == app.logs_selected {
            Style::default().bg(app.theme.highlight).fg(app.theme.bg).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.fg)
        };
        let status_color = match log.status {
            crate::types::FeedStatus::Healthy => app.theme.success,
            crate::types::FeedStatus::Warning => app.theme.warning,
            crate::types::FeedStatus::Error => app.theme.error,
            crate::types::FeedStatus::Disabled => app.theme.muted,
        };
        let feed_name = match app.feeds_list.iter().find(|ft| ft.feed.id == log.feed_id) {
            Some(ft) => ft.feed.name.clone(),
            None => format!("Feed #{}", log.feed_id),
        };
        let time_str = log.checked_at.format("%Y-%m-%d %H:%M:%S").to_string();
        Row::new(vec![
            Cell::from(time_str),
            Cell::from(feed_name),
            Cell::from(log.status.label()).style(Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
            Cell::from(log.error_message.as_deref().unwrap_or("—")),
        ]).style(style)
    }).collect();

    let table = Table::new(rows, vec![
        Constraint::Length(20), Constraint::Min(20), Constraint::Length(12), Constraint::Min(30),
    ])
    .header(header)
    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(app.theme.border)));
    f.render_widget(table, chunks[1]);

    let status = Paragraph::new("[f] Filter by feed  [c] Clear filter  [r] Refresh  [q] Back")
        .style(Style::default().fg(app.theme.muted));
    f.render_widget(status, chunks[2]);
}

pub fn handle_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('f') => {
            if let Some(ft) = app.feeds_list.get(app.feeds_selected) {
                app.logs_filter_feed = Some(ft.feed.id);
                app.refresh_logs();
            }
        }
        KeyCode::Char('c') => {
            app.logs_filter_feed = None;
            app.refresh_logs();
        }
        KeyCode::Char('r') => app.refresh_logs(),
        KeyCode::Down | KeyCode::Char('j') => {
            if !app.logs_list.is_empty() {
                app.logs_selected = (app.logs_selected + 1).min(app.logs_list.len() - 1);
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.logs_selected > 0 {
                app.logs_selected -= 1;
            }
        }
        _ => {}
    }
}

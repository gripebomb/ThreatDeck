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

    let title_text = if app.filter_active {
        format!("Logs | / {}", app.logs_filter)
    } else {
        let feed_filter = app
            .logs_filter_feed
            .and_then(|id| {
                app.feeds_list
                    .iter()
                    .find(|ft| ft.feed.id == id)
                    .map(|ft| ft.feed.name.clone())
            })
            .unwrap_or_else(|| "all feeds".into());
        format!(
            "Logs | Feed: {} | Filter: {}",
            feed_filter,
            if app.logs_filter.is_empty() {
                "none"
            } else {
                &app.logs_filter
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

    let header = Row::new(vec!["Time", "Feed", "Status", "Error"]).style(
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(app.theme.primary),
    );

    let mut table_state = TableState::default();
    table_state.select(Some(app.logs_selected));

    let rows: Vec<Row> = app
        .logs_list
        .iter()
        .map(|log| {
            let style = Style::default().fg(app.theme.fg);
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
                Cell::from(log.status.label()).style(
                    Style::default()
                        .fg(status_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(log.error_message.as_deref().unwrap_or("—")),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        vec![
            Constraint::Length(20),
            Constraint::Min(20),
            Constraint::Length(12),
            Constraint::Min(30),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border)),
    )
    .highlight_style(selected_style());
    f.render_stateful_widget(table, chunks[1], &mut table_state);

    let status_text = if app.filter_active {
        "-- FILTER -- Type search | [Enter] Keep | [Esc] Clear"
    } else {
        "-- NORMAL -- [1-8] Nav  [f] Cycle feed  [c] Clear feed  [r] Refresh  [/] Filter  [?] Help  [q] Quit"
    };
    let status = Paragraph::new(status_text).style(Style::default().fg(app.theme.muted));
    f.render_widget(status, chunks[2]);
}

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if let Some(motion) = motion_from_key(key, &mut app.pending_g) {
        app.logs_selected = move_selection(app.logs_selected, app.logs_list.len(), motion);
        return;
    }

    match key.code {
        KeyCode::Char('f') => {
            if !app.feeds_list.is_empty() {
                let current = app
                    .logs_filter_feed
                    .and_then(|id| app.feeds_list.iter().position(|ft| ft.feed.id == id));
                let next = current.map(|i| (i + 1) % app.feeds_list.len()).unwrap_or(0);
                app.logs_filter_feed = Some(app.feeds_list[next].feed.id);
                app.refresh_logs();
            }
        }
        KeyCode::Char('c') => {
            app.logs_filter_feed = None;
            app.refresh_logs();
        }
        KeyCode::Char('r') => app.refresh_logs(),
        KeyCode::Char('/') => {
            app.filter_active = true;
            app.input_mode = crate::app::InputMode::Typing;
        }
        _ => {}
    }
}

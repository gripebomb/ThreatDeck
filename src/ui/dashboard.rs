use ratatui::{Frame, layout::{Constraint, Direction, Layout, Rect}, style::{Style, Modifier}, widgets::{Block, Borders, Paragraph, Wrap}};
use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent};

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    // Title bar
    let title = Paragraph::new("ThreatStream — Dashboard")
        .style(Style::default().fg(app.theme.primary).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(app.theme.border)));
    f.render_widget(title, chunks[0]);

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(0), Constraint::Length(8)])
        .split(chunks[1]);

    // Stats row
    draw_stats(f, app, main_chunks[0]);

    // Middle: pie chart + recent alerts
    let mid_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(main_chunks[1]);

    draw_pie(f, app, mid_chunks[0]);
    draw_recent_alerts(f, app, mid_chunks[1]);

    // Bottom: sparkline placeholder
    draw_trend(f, app, main_chunks[2]);

    // Status bar
    let status = Paragraph::new("[r] Refresh  [1-7] Navigate  [?] Help  [q] Quit")
        .style(Style::default().fg(app.theme.muted));
    f.render_widget(status, chunks[2]);
}

fn draw_stats(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(25), Constraint::Percentage(25), Constraint::Percentage(25)])
        .split(area);

    let stats = [
        ("Feeds", app.dashboard_stats.total_feeds, format!("{} healthy", app.dashboard_stats.healthy_feeds)),
        ("Alerts", app.dashboard_stats.total_alerts, format!("{} unread", app.dashboard_stats.unread_alerts)),
        ("Keywords", app.dashboard_stats.total_keywords, "".to_string()),
        ("Health", (app.db.get_feed_health_ratio().unwrap_or(1.0) * 100.0) as i64, "% healthy".to_string()),
    ];

    for (i, (label, value, sub)) in stats.iter().enumerate() {
        let block = Block::default()
            .title(*label)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border));
        let text = format!("{}{}", value, sub);
        let para = Paragraph::new(text)
            .style(Style::default().fg(app.theme.fg).add_modifier(Modifier::BOLD))
            .block(block)
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(para, chunks[i]);
    }
}

fn draw_pie(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title("Criticality Distribution")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.border));

    let mut lines = vec![
        ratatui::text::Line::from(""),
    ];

    let total: i64 = app.dashboard_criticality_data.iter().map(|(_, c)| c).sum();

    for (crit, count) in &app.dashboard_criticality_data {
        let pct = if total > 0 { (*count as f64 / total as f64) * 100.0 } else { 0.0 };
        let color = crate::theme::criticality_color(app.theme, *crit);
        let bar_len = ((pct / 100.0) * 20.0) as usize;
        let bar = "█".repeat(bar_len);
        lines.push(ratatui::text::Line::from(vec![
            ratatui::text::Span::styled(format!("{:>10}: ", format!("{:?}", crit)), Style::default().fg(color).add_modifier(Modifier::BOLD)),
            ratatui::text::Span::styled(bar, Style::default().fg(color)),
            ratatui::text::Span::styled(format!(" {} ({:.1}%)", count, pct), Style::default().fg(app.theme.muted)),
        ]));
    }

    if app.dashboard_criticality_data.is_empty() {
        lines.push(ratatui::text::Line::from("No alerts yet.").style(Style::default().fg(app.theme.muted)));
    }

    let para = Paragraph::new(ratatui::text::Text::from(lines))
        .block(block)
        .wrap(Wrap { trim: true });
    f.render_widget(para, area);
}

fn draw_recent_alerts(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title("Recent Alerts")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.border));

    let mut lines = vec![ratatui::text::Line::from("")];

    if app.dashboard_recent_alerts.is_empty() {
        lines.push(ratatui::text::Line::from("No alerts yet.").style(Style::default().fg(app.theme.muted)));
    } else {
        for alert in &app.dashboard_recent_alerts {
            let color = crate::theme::criticality_color(app.theme, alert.alert.criticality);
            let time_str = crate::ui::utils::time_ago(alert.alert.detected_at);
            lines.push(ratatui::text::Line::from(vec![
                ratatui::text::Span::styled("█ ", Style::default().fg(color)),
                ratatui::text::Span::styled(
                    format!("[{}] {} — {}", alert.feed_name, alert.keyword_pattern, time_str),
                    Style::default().fg(app.theme.fg)
                ),
            ]));
        }
    }

    let para = Paragraph::new(ratatui::text::Text::from(lines))
        .block(block)
        .wrap(Wrap { trim: true });
    f.render_widget(para, area);
}

fn draw_trend(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title("7-Day Alert Trend")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.border));

    let text = match app.db.get_alert_trend(7) {
        Ok(data) if !data.is_empty() => {
            let max = data.iter().map(|(_, c)| *c).max().unwrap_or(1);
            let mut lines = vec![];
            for (day, count) in data {
                let bar_len = ((count as f64 / max as f64) * 30.0) as usize;
                let bar = "█".repeat(bar_len);
                lines.push(ratatui::text::Line::from(vec![
                    ratatui::text::Span::styled(format!("{} ", day), Style::default().fg(app.theme.muted)),
                    ratatui::text::Span::styled(bar, Style::default().fg(app.theme.primary)),
                    ratatui::text::Span::styled(format!(" {}", count), Style::default().fg(app.theme.fg)),
                ]));
            }
            ratatui::text::Text::from(lines)
        }
        _ => ratatui::text::Text::from("No data available."),
    };

    let para = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: true });
    f.render_widget(para, area);
}

pub fn handle_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('r') => app.refresh_dashboard(),
        _ => {}
    }
}

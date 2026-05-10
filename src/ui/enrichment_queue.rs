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
        format!("Enrichment Queue | / {}", app.enrichment_queue_filter)
    } else {
        format!(
            "Enrichment Queue | Jobs: {} | Filter: {}",
            app.enrichment_queue_list.len(),
            if app.enrichment_queue_filter.is_empty() {
                "none"
            } else {
                &app.enrichment_queue_filter
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

    let header = Row::new(vec![
        "Provider",
        "Indicator",
        "Type",
        "Status",
        "Attempts",
        "Next Attempt",
        "Last Error",
    ])
    .style(
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(app.theme.primary),
    );

    let mut table_state = TableState::default();
    table_state.select(Some(app.enrichment_queue_selected));

    let rows = app.enrichment_queue_list.iter().map(|job| {
        let status_color = match job.status.as_str() {
            "succeeded" => app.theme.success,
            "failed" => app.theme.error,
            "retrying" => app.theme.warning,
            "running" => app.theme.primary,
            _ => app.theme.fg,
        };
        Row::new(vec![
            Cell::from(truncate_chars(&job.provider_name, 24)),
            Cell::from(truncate_chars(&job.indicator_value, 56)),
            Cell::from(indicator_type_label(job.indicator_type)),
            Cell::from(job.status.clone()).style(
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from(job.attempt_count.to_string()),
            Cell::from(job.next_attempt_at.format("%Y-%m-%d %H:%M").to_string()),
            Cell::from(truncate_chars(
                job.error_message.as_deref().unwrap_or("-"),
                44,
            )),
        ])
        .style(Style::default().fg(app.theme.fg))
    });

    let table = Table::new(
        rows,
        vec![
            Constraint::Length(24),
            Constraint::Min(26),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Length(16),
            Constraint::Min(18),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border)),
    )
    .row_highlight_style(selected_style());
    f.render_stateful_widget(table, chunks[1], &mut table_state);

    let status_text = if app.filter_active {
        "-- FILTER -- Type search | [Enter] Keep | [Esc] Clear"
    } else {
        "-- NORMAL -- [1-9,0] Nav  [p] Process  [r] Refresh  [/] Filter  [?] Help  [q] Quit"
    };
    let status = Paragraph::new(status_text).style(Style::default().fg(app.theme.muted));
    f.render_widget(status, chunks[2]);
}

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if let Some(motion) = motion_from_key(key, &mut app.pending_g) {
        app.enrichment_queue_selected = move_selection(
            app.enrichment_queue_selected,
            app.enrichment_queue_list.len(),
            motion,
        );
        return;
    }

    match key.code {
        KeyCode::Char('p') => process_enrichment_queue(app),
        KeyCode::Char('r') => app.refresh_enrichment_queue(),
        KeyCode::Char('/') => {
            app.filter_active = true;
            app.input_mode = crate::app::InputMode::Typing;
        }
        _ => {}
    }
}

fn process_enrichment_queue(app: &mut App) {
    if !app.config.enrichment.enabled {
        app.set_notification(
            "Enrichment processing is disabled in settings".to_string(),
            crate::types::NotificationType::Warning,
        );
        return;
    }

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            app.set_notification(
                format!("Unable to start enrichment runtime: {}", e),
                crate::types::NotificationType::Error,
            );
            return;
        }
    };

    match runtime.block_on(crate::enrichment::run_enrichment_once(
        &app.db,
        &app.paths.data_dir,
        10,
    )) {
        Ok(processed) => {
            app.refresh_enrichment_queue();
            app.refresh_indicators();
            app.set_notification(
                format!("Processed {} enrichment job(s)", processed),
                crate::types::NotificationType::Success,
            );
        }
        Err(e) => {
            app.refresh_enrichment_queue();
            app.set_notification(
                format!("Unable to process enrichment queue: {}", e),
                crate::types::NotificationType::Error,
            );
        }
    }
}

fn truncate_chars(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_string()
    } else {
        format!(
            "{}...",
            value
                .chars()
                .take(max.saturating_sub(3))
                .collect::<String>()
        )
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

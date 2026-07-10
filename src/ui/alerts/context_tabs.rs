use crate::app::App;
use crate::types::{IndicatorViewModel, TriageEventViewModel};
use crate::ui::alerts::state::AlertContextTab;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs, Wrap},
    Frame,
};

/// Render the bottom-right context panel with three tabs: IOCs, Metadata,
/// and Triage History. The active tab is selected from `workbench_state`.
/// When `focused`, the panel border is drawn in the theme primary color.
pub fn draw_context_panel(f: &mut Frame, app: &App, area: Rect, focused: bool) {
    // Split: a fixed-height tab bar on top, content fills the remainder.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);
    let tab_area = chunks[0];
    let content_area = chunks[1];

    // --- Tab bar ---
    let titles = vec![
        Line::from("IOCs"),
        Line::from("Metadata"),
        Line::from("History"),
    ];
    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
                .border_style(if focused {
                    Style::default().fg(app.theme.primary)
                } else {
                    Style::default().fg(app.theme.border)
                }),
        )
        .select(match app.workbench_state.active_context_tab {
            AlertContextTab::Iocs => 0,
            AlertContextTab::Metadata => 1,
            AlertContextTab::TriageHistory => 2,
        })
        .style(Style::default().fg(app.theme.fg))
        .highlight_style(
            Style::default()
                .fg(app.theme.primary)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, tab_area);

    // --- Content area (left/right/bottom border, open at top to meet tab bar) ---
    let content_block = Block::default()
        .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
        .border_style(if focused {
            Style::default().fg(app.theme.primary)
        } else {
            Style::default().fg(app.theme.border)
        });
    let content_inner = content_block.inner(content_area);
    f.render_widget(content_block, content_area);

    // Tiny terminal safety: nothing fits, stop here.
    if content_inner.width == 0 || content_inner.height == 0 {
        return;
    }

    // No alert loaded -> empty state.
    let bundle = match app.workbench_bundle.as_ref() {
        None => {
            let empty =
                Paragraph::new("No alert selected.").style(Style::default().fg(app.theme.muted));
            f.render_widget(empty, content_inner);
            return;
        }
        Some(b) => b,
    };

    match app.workbench_state.active_context_tab {
        AlertContextTab::Iocs => draw_iocs(f, app, content_inner, &bundle.indicators),
        AlertContextTab::Metadata => draw_metadata(
            f,
            app,
            content_inner,
            bundle.detail.metadata_json.as_deref(),
        ),
        AlertContextTab::TriageHistory => {
            draw_triage_history(f, app, content_inner, &bundle.triage_events)
        }
    }
}

/// IOCs tab: a table of extracted indicators, or an empty-state message.
fn draw_iocs(f: &mut Frame, app: &App, area: Rect, indicators: &[IndicatorViewModel]) {
    if indicators.is_empty() {
        let empty = Paragraph::new("No indicators extracted for this alert.")
            .style(Style::default().fg(app.theme.muted));
        f.render_widget(empty, area);
        return;
    }

    let header = Row::new(vec!["Type", "Value", "Sightings", "Risk", "Reputation"]).style(
        Style::default()
            .fg(app.theme.primary)
            .add_modifier(Modifier::BOLD),
    );
    let selected = app.workbench_state.selected_ioc_index;
    let rows = indicators.iter().enumerate().map(|(i, ind)| {
        let style = if i == selected {
            Style::default()
                .fg(app.theme.primary)
                .bg(app.theme.surface)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.fg)
        };
        Row::new(vec![
            Cell::from(ind.indicator_type.as_str()),
            Cell::from(ind.normalized_value.as_str()),
            Cell::from(ind.sighting_count.to_string()),
            Cell::from(
                ind.risk_score
                    .map(|r| r.to_string())
                    .unwrap_or_else(|| "-".into()),
            ),
            Cell::from(
                ind.enrichments
                    .first()
                    .and_then(|e| e.reputation.as_ref())
                    .map(|s| s.as_str())
                    .unwrap_or("-"),
            ),
        ])
        .style(style)
    });
    let table = Table::new(
        rows,
        vec![
            Constraint::Length(12),
            Constraint::Min(20),
            Constraint::Length(9),
            Constraint::Length(6),
            Constraint::Length(12),
        ],
    )
    .header(header)
    .block(Block::default().title("Indicators"));
    f.render_widget(table, area);
}

/// Metadata tab: pretty-printed alert metadata JSON, or an empty-state message.
fn draw_metadata(f: &mut Frame, app: &App, area: Rect, metadata_json: Option<&str>) {
    let raw = match metadata_json {
        Some(s) if !s.is_empty() => s,
        _ => {
            let empty = Paragraph::new("No metadata available.")
                .style(Style::default().fg(app.theme.muted));
            f.render_widget(empty, area);
            return;
        }
    };

    let pretty = if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(raw) {
        serde_json::to_string_pretty(&parsed).unwrap_or_else(|_| raw.to_string())
    } else {
        raw.to_string()
    };

    let paragraph = Paragraph::new(pretty)
        .style(Style::default().fg(app.theme.fg))
        .wrap(Wrap { trim: false })
        .scroll((app.workbench_state.context_scroll, 0));
    f.render_widget(paragraph, area);
}

/// Triage History tab: a table of triage events, or an empty-state message.
fn draw_triage_history(f: &mut Frame, app: &App, area: Rect, events: &[TriageEventViewModel]) {
    if events.is_empty() {
        let empty = Paragraph::new("No triage history recorded.")
            .style(Style::default().fg(app.theme.muted));
        f.render_widget(empty, area);
        return;
    }

    let header = Row::new(vec!["Time", "Event", "Change", "Actor"]).style(
        Style::default()
            .fg(app.theme.primary)
            .add_modifier(Modifier::BOLD),
    );
    let rows = events.iter().map(|ev| {
        let mut change = match (&ev.old_value, &ev.new_value) {
            (Some(old), Some(new)) => format!("{} → {}", old, new),
            (None, Some(new)) => new.clone(),
            (Some(old), None) => format!("{} → (none)", old),
            (None, None) => String::new(),
        };
        if let Some(note) = &ev.note {
            change.push_str(&format!(" — {}", note));
        }

        Row::new(vec![
            Cell::from(ev.created_at.format("%Y-%m-%d %H:%M").to_string()),
            Cell::from(ev.event_type.as_str()),
            Cell::from(change),
            Cell::from(ev.actor.as_str()),
        ])
        .style(Style::default().fg(app.theme.fg))
    });
    let table = Table::new(
        rows,
        vec![
            Constraint::Length(16),
            Constraint::Length(16),
            Constraint::Min(20),
            Constraint::Length(12),
        ],
    )
    .header(header)
    .block(Block::default().title("Triage History"));
    f.render_widget(table, area);
}

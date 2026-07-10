use crate::app::App;
use crate::ui::list::{criticality_label, selected_style};
use crate::ui::utils::time_ago;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};

/// Width threshold (in cells) below which the list drops to a compact layout.
const COMPACT_WIDTH: u16 = 60;

/// Render the left alert-list pane of the split-pane workbench.
///
/// Shows a compact table of alerts (read marker, severity, status, title,
/// feed, age) with stateful selection. Collapses to read/severity/title on
/// narrow terminals. All data comes from `app.workbench_state` — no DB calls.
pub fn draw_alert_list(f: &mut Frame, app: &App, area: Rect, focused: bool) {
    let alerts = &app.workbench_state.alerts;
    let count = alerts.len();

    let title = if count > 0 {
        format!("Alerts ({})", count)
    } else {
        "Alerts".to_string()
    };
    let border_color = if focused {
        app.theme.primary
    } else {
        app.theme.border
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(title);

    // Empty state.
    if count == 0 {
        let empty = Paragraph::new("No alerts found.")
            .style(Style::default().fg(app.theme.muted))
            .block(block);
        f.render_widget(empty, area);
        return;
    }

    let compact = area.width < COMPACT_WIDTH;

    let header_cells: Vec<&str> = if compact {
        vec!["R", "Severity", "Title"]
    } else {
        vec!["R", "Severity", "Status", "Title", "Feed", "Age"]
    };
    let header = Row::new(header_cells).style(
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(app.theme.primary),
    );

    let rows: Vec<Row> = alerts
        .iter()
        .map(|item| {
            let read_mark = if item.read { "○" } else { "●" };
            let read_color = if item.read {
                app.theme.muted
            } else {
                app.theme.primary
            };
            let sev_color = crate::theme::criticality_color(app.theme, item.criticality);

            let mut cells = vec![
                Cell::from(read_mark).style(Style::default().fg(read_color)),
                Cell::from(criticality_label(item.criticality))
                    .style(Style::default().fg(sev_color).add_modifier(Modifier::BOLD)),
            ];
            if compact {
                cells.push(Cell::from(item.title.as_str()));
            } else {
                cells.push(
                    Cell::from(format!("{}", item.status)).style(Style::default().fg(app.theme.fg)),
                );
                cells.push(Cell::from(item.title.as_str()));
                cells.push(Cell::from(item.feed_name.as_str()));
                cells.push(
                    Cell::from(time_ago(item.detected_at))
                        .style(Style::default().fg(app.theme.muted)),
                );
            }
            Row::new(cells).style(Style::default().fg(app.theme.fg))
        })
        .collect();

    let widths = if compact {
        vec![
            Constraint::Length(3),
            Constraint::Length(10),
            Constraint::Min(10),
        ]
    } else {
        vec![
            Constraint::Length(3),
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Min(20),
            Constraint::Min(10),
            Constraint::Length(10),
        ]
    };

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(selected_style());

    let mut table_state = TableState::default();
    table_state.select(Some(app.workbench_state.selected_alert_index));
    // NOTE: workbench_state.list_scroll is intentionally unused here; TableState
    // exposes no scroll API in ratatui 0.30, so vertical scroll is deferred.
    f.render_stateful_widget(table, area, &mut table_state);
}

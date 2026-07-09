//! Left alert-list pane renderer (Ticket 04).
//!
//! Renders [`AlertListItem`] view models inside the layout engine's left
//! rectangle. Pure presentation: it consumes view models and workbench state and
//! never touches storage or SQL. The caller (page coordinator / Phase 3 wiring)
//! is responsible for loading the items via the app service.
//!
//! See `tickets/04-alert-list-pane.md` and `docs/ARCHITECTURE.md`.

use chrono::{DateTime, Utc};
use ratatui::{
    layout::{Alignment, Constraint, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::theme::{criticality_color, Theme};
use crate::types::Criticality;
use crate::ui::alert_workbench::{AlertListItem, AlertPane, AlertWorkbenchState};
use crate::ui::list::selected_style;

/// Render the left alert-list pane into `area`.
///
/// - Empty list → a muted empty-state message (filter-aware).
/// - Non-empty → a table with unread marker, severity/status **text** labels,
///   feed name, alert title, and detected age, with the selected row
///   highlighted and a focused/unfocused border.
pub fn draw_alert_list(
    f: &mut Frame,
    area: Rect,
    items: &[AlertListItem],
    state: &AlertWorkbenchState,
    theme: &Theme,
) {
    let focused = state.focused_pane == AlertPane::AlertList;
    let border_color = if focused { theme.primary } else { theme.border };
    let filter_tag = if state.alert_filter.is_empty() {
        String::new()
    } else {
        " (filtered)".to_string()
    };
    let title = format!(" Alerts ({}){} ", items.len(), filter_tag);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(title)
        .title_style(
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD),
        );

    if items.is_empty() {
        let (msg, color) = if let Some(err) = &state.last_error {
            (format!("⚠ Couldn't load alerts:\n\n{err}"), theme.error)
        } else if state.alert_filter.is_empty() {
            (
                "No alerts to show.\n\nAdjust filters or refresh to load alerts.".to_string(),
                theme.muted,
            )
        } else {
            (
                "No alerts match the current filter.\n\nClear the filter to see more.".to_string(),
                theme.muted,
            )
        };
        let empty = Paragraph::new(msg)
            .style(Style::default().fg(color))
            .alignment(Alignment::Center)
            .block(block);
        f.render_widget(empty, area);
        return;
    }

    // Compact layout drops the Feed column when the pane is narrow so the title
    // and triage cues stay readable.
    let compact = area.width < COMPACT_WIDTH_THRESHOLD;
    let rows = items
        .iter()
        .map(|item| build_row(item, compact, theme))
        .collect::<Vec<_>>();
    let header = build_header(compact, theme);

    let mut table_state = TableState::default();
    table_state.select(Some(state.selected_alert_index.min(items.len() - 1)));
    // `select` resets the offset, so apply scroll afterwards.
    *table_state.offset_mut() = state.alert_list_scroll;

    let table = Table::new(rows, column_constraints(compact))
        .header(header)
        .block(block)
        .row_highlight_style(selected_style());
    f.render_stateful_widget(table, area, &mut table_state);
}

/// Pane width (including borders) below which the compact layout is used.
const COMPACT_WIDTH_THRESHOLD: u16 = 48;

fn build_header(compact: bool, theme: &Theme) -> Row<'static> {
    let labels: Vec<&'static str> = if compact {
        vec!["", "Sev", "Stat", "Alert", "When"]
    } else {
        vec!["", "Sev", "Stat", "Feed", "Alert", "When"]
    };
    Row::new(labels)
        .style(
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD),
        )
        .height(1)
}

fn column_constraints(compact: bool) -> Vec<Constraint> {
    if compact {
        vec![
            Constraint::Length(2),
            Constraint::Length(5),
            Constraint::Length(6),
            Constraint::Min(8),
            Constraint::Length(8),
        ]
    } else {
        vec![
            Constraint::Length(2),
            Constraint::Length(5),
            Constraint::Length(6),
            Constraint::Length(12),
            Constraint::Min(8),
            Constraint::Length(10),
        ]
    }
}

fn build_row(item: &AlertListItem, compact: bool, theme: &Theme) -> Row<'static> {
    // Unread marker (text glyph, not colour-only).
    let read_mark = if item.read { "○" } else { "●" };
    let read_style = if item.read {
        Style::default().fg(theme.muted)
    } else {
        Style::default().fg(theme.primary)
    };

    // Severity as a text label; colour is secondary emphasis only.
    let sev = severity_short(item.severity);
    let sev_style = Style::default()
        .fg(criticality_color(theme, item.severity))
        .add_modifier(Modifier::BOLD);

    let status = format!("{}", item.status);
    let title = item
        .title
        .clone()
        .unwrap_or_else(|| "(untitled)".to_string());
    let detected = relative_time(item.detected_at);

    let mut cells: Vec<Cell<'static>> = vec![
        Cell::from(read_mark).style(read_style),
        Cell::from(sev).style(sev_style),
        Cell::from(status),
    ];
    if !compact {
        cells.push(Cell::from(item.feed_name.clone()));
    }
    cells.push(Cell::from(title));
    cells.push(Cell::from(detected));

    Row::new(cells).style(Style::default().fg(theme.fg))
}

/// Short, colour-independent severity word for the list (Low/Med/High/Crit).
fn severity_short(criticality: Criticality) -> &'static str {
    match criticality {
        Criticality::Low => "Low",
        Criticality::Medium => "Med",
        Criticality::High => "High",
        Criticality::Critical => "Crit",
    }
}

/// Compact relative age for the "When" column. Deterministic only up to the
/// age bucket; render tests assert on labels/structure rather than exact times.
fn relative_time(dt: DateTime<Utc>) -> String {
    let secs = Utc::now().signed_duration_since(dt).num_seconds().max(0);
    if secs < 60 {
        "now".to_string()
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86_400 {
        format!("{}h", secs / 3600)
    } else if secs < 2_592_000 {
        format!("{}d", secs / 86_400)
    } else {
        dt.format("%Y-%m-%d").to_string()
    }
}

#[cfg(test)]
mod tests {
    //! TUI smoke tests for the alert list pane (Ticket 04).
    //!
    //! Mirrors the TESTING_CHECKLIST smoke tests: render with no alerts, one
    //! alert, many alerts, plus compact narrow mode and focused border.
    use super::*;
    use crate::theme::get_theme;
    use crate::types::{AlertDisposition, AlertStatus};
    use chrono::Utc;
    use ratatui::{backend::TestBackend, Terminal};

    fn item(id: i64, title: &str, severity: Criticality, status: AlertStatus) -> AlertListItem {
        AlertListItem {
            id,
            title: Some(title.to_string()),
            severity,
            status,
            disposition: AlertDisposition::Unknown,
            feed_name: "IOCFeed".to_string(),
            keyword_pattern: "ransomware".to_string(),
            read: id % 2 == 0,
            detected_at: Utc::now(),
            tags: Vec::new(),
        }
    }

    fn state_with_selection(index: usize, focused: bool) -> AlertWorkbenchState {
        let mut s = AlertWorkbenchState::new();
        s.selected_alert_index = index;
        s.focused_pane = if focused {
            AlertPane::AlertList
        } else {
            AlertPane::AlertDetails
        };
        s
    }

    /// Render the pane into a fresh terminal of the given size and return the
    /// buffer contents as one string per row.
    fn render_rows(
        items: &[AlertListItem],
        state: &AlertWorkbenchState,
        w: u16,
        h: u16,
    ) -> Vec<String> {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw_alert_list(f, f.area(), items, state, get_theme("dark")))
            .unwrap();
        let buf = terminal.backend().buffer();
        (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| buf[(x, y)].symbol().chars().next().unwrap_or(' '))
                    .collect::<String>()
            })
            .collect()
    }

    /// Distinct y-rows containing at least one cell styled REVERSED (i.e. the
    /// highlighted selection row).
    fn reversed_rows(
        items: &[AlertListItem],
        state: &AlertWorkbenchState,
        w: u16,
        h: u16,
    ) -> Vec<u16> {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw_alert_list(f, f.area(), items, state, get_theme("dark")))
            .unwrap();
        let buf = terminal.backend().buffer();
        let mut rows: Vec<u16> = Vec::new();
        for y in 0..h {
            for x in 0..w {
                if buf[(x, y)]
                    .style()
                    .add_modifier
                    .contains(Modifier::REVERSED)
                    && !rows.contains(&y)
                {
                    rows.push(y);
                }
            }
        }
        rows
    }

    fn joined(rows: &[String]) -> String {
        rows.join("\n")
    }

    // ── Empty state ──────────────────────────────────────────────────────────

    #[test]
    fn renders_empty_state_when_no_alerts() {
        let state = state_with_selection(0, true);
        let rows = render_rows(&[], &state, 60, 12);
        let text = joined(&rows);
        assert!(
            text.contains("No alerts to show"),
            "empty message missing:\n{text}"
        );
        // Title still shows count 0.
        assert!(text.contains("Alerts (0)"), "title missing:\n{text}");
        assert!(reversed_rows(&[], &state, 60, 12).is_empty());
    }

    #[test]
    fn empty_state_mentions_filter_when_active() {
        let mut state = state_with_selection(0, true);
        state.alert_filter.text = "ransom".into();
        let rows = render_rows(&[], &state, 60, 12);
        let text = joined(&rows);
        assert!(
            text.contains("match the current filter"),
            "filter text missing:\n{text}"
        );
        assert!(
            text.contains("(filtered)"),
            "filtered title tag missing:\n{text}"
        );
    }

    #[test]
    fn empty_state_shows_load_error_when_present() {
        let mut state = AlertWorkbenchState::new();
        state.last_error = Some("database is locked".into());
        let text = joined(&render_rows(&[], &state, 60, 12));
        assert!(
            text.contains("Couldn't load alerts"),
            "error message missing:\n{text}"
        );
        assert!(
            text.contains("database is locked"),
            "error detail missing:\n{text}"
        );
    }

    #[test]
    fn empty_state_without_error_still_mentions_filters() {
        let state = AlertWorkbenchState::new();
        let text = joined(&render_rows(&[], &state, 60, 12));
        assert!(text.contains("No alerts"), "{text}");
        assert!(!text.contains("Couldn't load"), "{text}");
    }

    // ── One alert ────────────────────────────────────────────────────────────

    #[test]
    fn renders_severity_and_status_as_text_labels() {
        let items = vec![item(
            1,
            "Ransomware spike",
            Criticality::High,
            AlertStatus::New,
        )];
        let state = state_with_selection(0, true);
        let text = joined(&render_rows(&items, &state, 70, 12));
        // Severity and status must be readable text, not colour-only.
        assert!(text.contains("High"), "severity label missing:\n{text}");
        assert!(text.contains("New"), "status label missing:\n{text}");
        assert!(text.contains("Ransomware spike"), "title missing:\n{text}");
        assert!(text.contains("IOCFeed"), "feed name missing:\n{text}");
        assert!(text.contains("Alerts (1)"), "title missing:\n{text}");
    }

    #[test]
    fn renders_unread_marker_for_unread_alert() {
        // item id 1 → read = (1 % 2 == 0) == false → unread → "●".
        let items = vec![item(
            1,
            "Unread alert",
            Criticality::Medium,
            AlertStatus::New,
        )];
        let state = state_with_selection(0, true);
        let text = joined(&render_rows(&items, &state, 70, 12));
        assert!(text.contains('●'), "unread marker missing:\n{text}");
    }

    // ── Many alerts + selection highlight ─────────────────────────────────────

    #[test]
    fn renders_many_alerts_and_highlights_selected_row() {
        let items = vec![
            item(1, "Alpha alert", Criticality::Low, AlertStatus::New),
            item(
                2,
                "Beta alert",
                Criticality::High,
                AlertStatus::Acknowledged,
            ),
            item(
                3,
                "Gamma alert",
                Criticality::Critical,
                AlertStatus::Escalated,
            ),
        ];

        // Selecting index 0 vs index 2 must highlight different rows.
        let rows0 = reversed_rows(&items, &state_with_selection(0, true), 80, 16);
        let rows2 = reversed_rows(&items, &state_with_selection(2, true), 80, 16);

        assert_eq!(
            rows0.len(),
            1,
            "exactly one row should be highlighted (sel 0)"
        );
        assert_eq!(
            rows2.len(),
            1,
            "exactly one row should be highlighted (sel 2)"
        );
        assert_ne!(rows0[0], rows2[0], "highlight must follow the selection");

        // All three titles render.
        let text = joined(&render_rows(&items, &state_with_selection(1, true), 80, 16));
        assert!(
            text.contains("Alpha alert")
                && text.contains("Beta alert")
                && text.contains("Gamma alert")
        );
        assert!(text.contains("Alerts (3)"));
    }

    // ── Compact narrow mode ───────────────────────────────────────────────────

    #[test]
    fn compact_mode_drops_feed_column_when_narrow() {
        let items = vec![item(1, "Narrow alert", Criticality::High, AlertStatus::New)];
        let state = state_with_selection(0, true);

        // Wide: Feed header and value are present.
        let wide = joined(&render_rows(&items, &state, 80, 12));
        assert!(
            wide.contains("Feed"),
            "wide layout should show Feed header:\n{wide}"
        );

        // Narrow: the Feed header disappears (compact layout).
        let narrow = joined(&render_rows(&items, &state, 34, 12));
        assert!(
            !narrow.lines().any(|l| l.contains("Feed")),
            "compact layout should drop the Feed column:\n{narrow}"
        );
        // Core cues still present in compact mode (title may truncate to fit).
        assert!(narrow.contains("High"));
        assert!(narrow.contains("Narrow"));
    }

    // ── Focused vs unfocused border ───────────────────────────────────────────

    #[test]
    fn focused_border_uses_primary_color() {
        let items = vec![item(1, "Border alert", Criticality::High, AlertStatus::New)];

        let focused_rows = reversed_rows(&items, &state_with_selection(0, true), 60, 10);
        let unfocused_rows = reversed_rows(&items, &state_with_selection(0, false), 60, 10);

        // Selection highlight is present in both (selection is independent of focus).
        assert_eq!(focused_rows.len(), 1);
        assert_eq!(unfocused_rows.len(), 1);

        // Title and content render regardless of focus.
        let focused_text = joined(&render_rows(&items, &state_with_selection(0, true), 60, 10));
        let unfocused_text = joined(&render_rows(
            &items,
            &state_with_selection(0, false),
            60,
            10,
        ));
        assert!(focused_text.contains("Border alert"));
        assert!(unfocused_text.contains("Border alert"));
    }

    // ── Tiny terminal safety ──────────────────────────────────────────────────

    #[test]
    fn does_not_panic_on_tiny_area() {
        let items = vec![item(1, "Tiny", Criticality::High, AlertStatus::New)];
        let state = state_with_selection(0, true);
        // Should render without panicking even at minuscule sizes.
        let _ = render_rows(&items, &state, 8, 3);
        let _ = render_rows(&[], &state, 6, 3);
    }
}

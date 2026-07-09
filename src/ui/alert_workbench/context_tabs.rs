//! Bottom-right context panel with tabs (Ticket 06).
//!
//! Renders the [`AlertWorkbenchBundle`]'s context data inside the layout
//! engine's bottom-right rectangle: a tab bar driven by
//! [`AlertContextTab`] switching between IOCs, Metadata (pretty JSON),
//! Triage History, plus future-ready Enrichment and Raw Content tabs.
//! Pure presentation: no storage access.
//!
//! See `tickets/06-context-panel-tabs.md` and `docs/ARCHITECTURE.md`.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Tabs, Wrap},
    Frame,
};

use crate::theme::Theme;
use crate::ui::alert_workbench::{
    AlertContextTab, AlertPane, AlertWorkbenchBundle, AlertWorkbenchState, IndicatorViewModel,
    TriageEventViewModel,
};

/// Render the bottom-right context panel into `area`.
///
/// Precedence: error > no-selection empty state > tabbed content. The active
/// tab follows `state.bottom_tab`; scroll follows `state.bottom_detail_scroll`;
/// the focused border follows `focused_pane`.
pub fn draw_context_panel(
    f: &mut Frame,
    area: Rect,
    bundle: Option<&AlertWorkbenchBundle>,
    state: &AlertWorkbenchState,
    theme: &Theme,
) {
    let focused = state.focused_pane == AlertPane::ContextPanel;
    let border_color = if focused { theme.primary } else { theme.border };
    let focus = if focused { "◆ " } else { "" };
    let title = format!(" {}Context: {} ", focus, state.bottom_tab.label());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(title)
        .title_style(
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD),
        );

    // Precedence: error > no-selection > tabbed content.
    if let Some(err) = &state.last_error {
        let msg = format!("⚠ Couldn't load this alert's context:\n\n{err}");
        render_block_message(f, area, block, &msg, theme.error);
        return;
    }

    let Some(bundle) = bundle else {
        render_block_message(
            f,
            area,
            block,
            "No alert selected.\n\nSelect an alert to view its indicators, metadata, and history.",
            theme.muted,
        );
        return;
    };

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Tab bar (with a bottom divider) + scrollable body.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(0)])
        .split(inner);
    draw_tab_bar(f, chunks[0], state.bottom_tab, theme);

    let body = chunks[1];
    let scroll = state.bottom_detail_scroll;
    match state.bottom_tab {
        AlertContextTab::Indicators => {
            draw_indicators_tab(f, body, &bundle.indicators, scroll, theme);
        }
        AlertContextTab::Metadata => {
            draw_metadata_tab(f, body, bundle.metadata_json.as_deref(), scroll, theme);
        }
        AlertContextTab::TriageHistory => {
            draw_history_tab(f, body, &bundle.triage_history, scroll, theme);
        }
        AlertContextTab::Enrichment => render_centered(
            f,
            body,
            "Enrichment\n\nPer-indicator enrichment is shown inline on the IOCs tab.\nA dedicated cross-indicator enrichment view is coming soon.",
            theme.muted,
        ),
        AlertContextTab::RawContent => match bundle.raw_content.as_deref() {
            Some(raw) if !raw.trim().is_empty() => {
                // Clamp scroll by wrapped visual rows so the view can't run
                // past the content (and a long wrapped line stays scrollable).
                let content_lines = wrapped_line_count(raw, body.width);
                let max_scroll = content_lines.saturating_sub(body.height);
                let p = Paragraph::new(raw)
                    .style(Style::default().fg(theme.fg))
                    .wrap(Wrap { trim: false })
                    .scroll((scroll.min(max_scroll), 0));
                f.render_widget(p, body);
            }
            _ => render_centered(
                f,
                body,
                "Raw Content\n\nNo raw source content is available for this alert.",
                theme.muted,
            ),
        },
    }
}

/// Render the tab bar with the active tab highlighted.
fn draw_tab_bar(f: &mut Frame, area: Rect, active: AlertContextTab, theme: &Theme) {
    let titles: Vec<&str> = AlertContextTab::ALL.iter().map(|t| t.label()).collect();
    let selected = AlertContextTab::ALL
        .iter()
        .position(|&t| t == active)
        .unwrap_or(0);
    let tabs = Tabs::new(titles)
        .select(selected)
        .highlight_style(
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::REVERSED | Modifier::BOLD),
        )
        .divider(" ")
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(theme.border)),
        );
    f.render_widget(tabs, area);
}

fn draw_indicators_tab(
    f: &mut Frame,
    area: Rect,
    indicators: &[IndicatorViewModel],
    scroll: u16,
    theme: &Theme,
) {
    if indicators.is_empty() {
        render_centered(
            f,
            area,
            "No indicators extracted for this alert.",
            theme.muted,
        );
        return;
    }

    let header = Row::new(vec!["Type", "Value", "Sight", "Reputation"]).style(
        Style::default()
            .fg(theme.primary)
            .add_modifier(Modifier::BOLD),
    );
    let rows = indicators.iter().map(|i| {
        let reputation = i
            .enrichment
            .first()
            .and_then(|e| e.reputation.clone().or_else(|| e.verdict.clone()))
            .unwrap_or_else(|| "—".to_string());
        Row::new(vec![
            Cell::from(i.type_label()),
            Cell::from(i.normalized_value.clone()),
            Cell::from(i.sighting_count.to_string()),
            Cell::from(reputation),
        ])
        .style(Style::default().fg(theme.fg))
    });

    let table = Table::new(
        rows,
        vec![
            Constraint::Length(7),
            Constraint::Min(16),
            Constraint::Length(6),
            Constraint::Length(12),
        ],
    )
    .header(header);

    // Clamp the row offset so the table can't scroll past the last row
    // (one row is the header).
    let visible_rows = (area.height.saturating_sub(1)) as usize;
    let max_offset = indicators.len().saturating_sub(visible_rows);
    let mut table_state = TableState::default();
    *table_state.offset_mut() = (scroll as usize).min(max_offset);
    f.render_stateful_widget(table, area, &mut table_state);
}

fn draw_metadata_tab(
    f: &mut Frame,
    area: Rect,
    metadata: Option<&str>,
    scroll: u16,
    theme: &Theme,
) {
    let Some(raw) = metadata.map(str::trim).filter(|s| !s.is_empty()) else {
        render_centered(f, area, "No metadata for this alert.", theme.muted);
        return;
    };
    let pretty = pretty_json(raw);
    // Count wrapped visual rows (not logical lines) so a long JSON value that
    // wraps across many rows can be scrolled through to the bottom.
    let content_lines = wrapped_line_count(&pretty, area.width);
    let max_scroll = content_lines.saturating_sub(area.height);
    let paragraph = Paragraph::new(pretty)
        .style(Style::default().fg(theme.fg))
        .wrap(Wrap { trim: false })
        .scroll((scroll.min(max_scroll), 0));
    f.render_widget(paragraph, area);
}

fn draw_history_tab(
    f: &mut Frame,
    area: Rect,
    history: &[TriageEventViewModel],
    scroll: u16,
    theme: &Theme,
) {
    if history.is_empty() {
        render_centered(f, area, "No triage history recorded.", theme.muted);
        return;
    }

    let muted = Style::default().fg(theme.muted);
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(history.len());
    for event in history {
        let ts = event.created_at.format("%Y-%m-%d %H:%M");
        let change = match (&event.old_value, &event.new_value) {
            (Some(old), Some(new)) => format!("{old} → {new}"),
            (None, Some(new)) => format!("→ {new}"),
            (Some(old), None) => format!("{old} → (none)"),
            (None, None) => String::new(),
        };
        let note = event
            .note
            .as_deref()
            .map(|n| format!("  — {n}"))
            .unwrap_or_default();
        lines.push(Line::from(vec![
            Span::styled(format!("[{ts}] "), muted),
            Span::styled(
                format!("{:<16}", event.event_type),
                Style::default().fg(theme.primary),
            ),
            Span::styled(format!(" {change}{note}"), Style::default().fg(theme.fg)),
        ]));
    }

    let content_lines = lines.len() as u16;
    let max_scroll = content_lines.saturating_sub(area.height);
    let paragraph = Paragraph::new(lines)
        .style(Style::default().fg(theme.fg))
        .scroll((scroll.min(max_scroll), 0));
    f.render_widget(paragraph, area);
}

/// Pretty-print a JSON string; fall back to the raw text if it is not valid JSON.
fn pretty_json(raw: &str) -> String {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|value| serde_json::to_string_pretty(&value).ok())
        .unwrap_or_else(|| raw.to_string())
}

/// Number of visual (wrapped) rows `text` occupies when rendered into a column
/// of `width` cells. Used to clamp scroll on `.wrap()`-enabled paragraphs
/// (metadata / raw content), where the logical line count undercounts the
/// rendered height and the bottom rows can't be scrolled into view. Display
/// width via `Span::width` so wide characters count as 2 cells.
fn wrapped_line_count(text: &str, width: u16) -> u16 {
    let w = (width as usize).max(1);
    let mut total: u16 = 0;
    for line in text.lines() {
        let line_w = Span::from(line).width();
        let rows = if line_w == 0 { 1 } else { line_w.div_ceil(w) };
        total = total.saturating_add(rows as u16);
    }
    // Empty text still renders as a single (blank) row.
    total.max(1)
}

fn render_block_message(
    f: &mut Frame,
    area: Rect,
    block: Block<'_>,
    msg: &str,
    color: ratatui::style::Color,
) {
    let p = Paragraph::new(msg)
        .style(Style::default().fg(color))
        .alignment(Alignment::Center)
        .block(block);
    f.render_widget(p, area);
}

fn render_centered(f: &mut Frame, area: Rect, msg: &str, color: ratatui::style::Color) {
    let p = Paragraph::new(msg)
        .style(Style::default().fg(color))
        .alignment(Alignment::Center);
    f.render_widget(p, area);
}

#[cfg(test)]
mod tests {
    //! TUI smoke tests for the context panel (Ticket 06).
    use super::*;
    use crate::theme::get_theme;
    use sentinel_ioc::IndicatorType;

    fn indicator(value: &str, kind: IndicatorType, reputation: Option<&str>) -> IndicatorViewModel {
        IndicatorViewModel {
            id: 1,
            indicator_type: kind,
            value: value.into(),
            normalized_value: value.into(),
            sighting_count: 3,
            confidence: Some(80),
            risk: Some(50),
            enrichment: reputation
                .map(
                    |r| crate::ui::alert_workbench::view_models::EnrichmentViewModel {
                        provider_id: 1,
                        status: "succeeded".into(),
                        reputation: Some(r.into()),
                        score: Some(95),
                        verdict: None,
                        summary: None,
                        fetched_at: chrono::Utc::now(),
                    },
                )
                .into_iter()
                .collect(),
        }
    }

    fn history_event() -> TriageEventViewModel {
        TriageEventViewModel {
            id: 1,
            event_type: "status_changed".into(),
            old_value: Some("New".into()),
            new_value: Some("Acknowledged".into()),
            note: Some("ack by analyst".into()),
            actor: Some("local".into()),
            created_at: chrono::Utc::now(),
        }
    }

    fn bundle_with(
        indicators: Vec<IndicatorViewModel>,
        metadata: Option<&str>,
        history: Vec<TriageEventViewModel>,
    ) -> AlertWorkbenchBundle {
        AlertWorkbenchBundle {
            detail: None,
            indicators,
            metadata_json: metadata.map(str::to_string),
            triage_history: history,
            raw_content: None,
        }
    }

    fn render_text(
        bundle: Option<&AlertWorkbenchBundle>,
        state: &AlertWorkbenchState,
        w: u16,
        h: u16,
    ) -> String {
        let backend = ratatui::backend::TestBackend::new(w, h);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw_context_panel(f, f.area(), bundle, state, get_theme("dark")))
            .unwrap();
        let buf = terminal.backend().buffer();
        (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| buf[(x, y)].symbol().chars().next().unwrap_or(' '))
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn state_on_tab(tab: AlertContextTab) -> AlertWorkbenchState {
        let mut s = AlertWorkbenchState::new();
        s.focused_pane = AlertPane::ContextPanel;
        s.bottom_tab = tab;
        s
    }

    // ── Tab bar ──────────────────────────────────────────────────────────────

    #[test]
    fn tab_bar_renders_all_tabs() {
        let bundle = bundle_with(vec![], None, vec![]);
        let text = render_text(
            Some(&bundle),
            &state_on_tab(AlertContextTab::Indicators),
            80,
            16,
        );
        for label in ["IOCs", "Metadata", "Enrichment", "History", "Raw"] {
            assert!(text.contains(label), "tab label {label} missing:\n{text}");
        }
    }

    #[test]
    fn title_reflects_active_tab() {
        let bundle = bundle_with(vec![], None, vec![]);
        let iocs = render_text(
            Some(&bundle),
            &state_on_tab(AlertContextTab::Indicators),
            80,
            16,
        );
        assert!(iocs.contains("Context: IOCs"));
        let hist = render_text(
            Some(&bundle),
            &state_on_tab(AlertContextTab::TriageHistory),
            80,
            16,
        );
        assert!(hist.contains("Context: History"));
    }

    // ── Tab body switching ───────────────────────────────────────────────────

    #[test]
    fn selecting_tab_switches_body_content() {
        let bundle = bundle_with(
            vec![indicator("CVE-2025-12345", IndicatorType::Cve, None)],
            Some(r#"{"a":1}"#),
            vec![history_event()],
        );

        let indicators = render_text(
            Some(&bundle),
            &state_on_tab(AlertContextTab::Indicators),
            80,
            16,
        );
        assert!(
            indicators.contains("CVE-2025-12345"),
            "IOCs body missing:\n{indicators}"
        );

        let metadata = render_text(
            Some(&bundle),
            &state_on_tab(AlertContextTab::Metadata),
            80,
            16,
        );
        assert!(
            metadata.contains("\"a\""),
            "metadata body missing:\n{metadata}"
        );

        let history = render_text(
            Some(&bundle),
            &state_on_tab(AlertContextTab::TriageHistory),
            80,
            16,
        );
        assert!(
            history.contains("status_changed"),
            "history body missing:\n{history}"
        );
    }

    // ── IOCs tab ─────────────────────────────────────────────────────────────

    #[test]
    fn indicators_tab_renders_data_and_empty_state() {
        let with_data = bundle_with(
            vec![indicator(
                "http://bad.example/drop",
                IndicatorType::Url,
                Some("Malicious"),
            )],
            None,
            vec![],
        );
        let text = render_text(
            Some(&with_data),
            &state_on_tab(AlertContextTab::Indicators),
            80,
            16,
        );
        assert!(text.contains("URL"), "type label missing:\n{text}");
        assert!(text.contains("bad.example/drop"), "value missing:\n{text}");
        assert!(text.contains("Malicious"), "reputation missing:\n{text}");

        let empty = bundle_with(vec![], None, vec![]);
        let text = render_text(
            Some(&empty),
            &state_on_tab(AlertContextTab::Indicators),
            80,
            16,
        );
        assert!(
            text.contains("No indicators"),
            "empty state missing:\n{text}"
        );
    }

    // ── Metadata tab ─────────────────────────────────────────────────────────

    #[test]
    fn metadata_tab_renders_pretty_json() {
        let bundle = bundle_with(vec![], Some(r#"{"b":2,"a":1}"#), vec![]);
        let text = render_text(
            Some(&bundle),
            &state_on_tab(AlertContextTab::Metadata),
            80,
            16,
        );
        // Pretty JSON is multi-line and indented; both keys appear.
        assert!(
            text.contains("\"a\"") && text.contains("\"b\""),
            "keys missing:\n{text}"
        );
        assert!(text.contains("\n"), "pretty JSON should be multi-line");
    }

    #[test]
    fn metadata_tab_invalid_json_falls_back_to_raw() {
        let bundle = bundle_with(vec![], Some("{not valid json"), vec![]);
        let text = render_text(
            Some(&bundle),
            &state_on_tab(AlertContextTab::Metadata),
            80,
            16,
        );
        assert!(
            text.contains("not valid json"),
            "raw fallback missing:\n{text}"
        );
    }

    #[test]
    fn metadata_tab_empty_state() {
        let bundle = bundle_with(vec![], None, vec![]);
        let text = render_text(
            Some(&bundle),
            &state_on_tab(AlertContextTab::Metadata),
            80,
            16,
        );
        assert!(
            text.contains("No metadata"),
            "empty metadata state missing:\n{text}"
        );
    }

    // ── History tab ──────────────────────────────────────────────────────────

    #[test]
    fn history_tab_renders_events_and_empty_state() {
        let with_events = bundle_with(vec![], None, vec![history_event()]);
        let text = render_text(
            Some(&with_events),
            &state_on_tab(AlertContextTab::TriageHistory),
            80,
            16,
        );
        assert!(
            text.contains("status_changed"),
            "event type missing:\n{text}"
        );
        assert!(text.contains("New"), "old value missing:\n{text}");
        assert!(text.contains("Acknowledged"), "new value missing:\n{text}");
        assert!(text.contains("ack by analyst"), "note missing:\n{text}");

        let empty = bundle_with(vec![], None, vec![]);
        let text = render_text(
            Some(&empty),
            &state_on_tab(AlertContextTab::TriageHistory),
            80,
            16,
        );
        assert!(
            text.contains("No triage history"),
            "empty history state missing:\n{text}"
        );
    }

    // ── Future-ready placeholders ────────────────────────────────────────────

    #[test]
    fn enrichment_tab_shows_placeholder() {
        let bundle = bundle_with(vec![], None, vec![]);
        let text = render_text(
            Some(&bundle),
            &state_on_tab(AlertContextTab::Enrichment),
            80,
            16,
        );
        assert!(
            text.contains("IOCs tab"),
            "enrichment placeholder missing:\n{text}"
        );
    }

    #[test]
    fn raw_tab_shows_placeholder_when_empty() {
        let bundle = bundle_with(vec![], None, vec![]);
        let text = render_text(
            Some(&bundle),
            &state_on_tab(AlertContextTab::RawContent),
            80,
            16,
        );
        assert!(
            text.contains("No raw source content"),
            "raw placeholder missing:\n{text}"
        );
    }

    // ── No selection / loading ───────────────────────────────────────────────

    #[test]
    fn shows_empty_state_when_no_selection() {
        let text = render_text(None, &state_on_tab(AlertContextTab::Indicators), 80, 16);
        assert!(
            text.contains("No alert selected"),
            "no-selection state missing:\n{text}"
        );
    }

    #[test]
    fn shows_error_state_when_error_set() {
        let mut state = state_on_tab(AlertContextTab::Indicators);
        state.last_error = Some("database locked".into());
        let bundle = bundle_with(vec![], None, vec![]);
        let text = render_text(Some(&bundle), &state, 80, 16);
        assert!(
            text.contains("Couldn't load"),
            "error state missing:\n{text}"
        );
        assert!(
            text.contains("database locked"),
            "error detail missing:\n{text}"
        );
    }

    // ── Focused border + tiny safety ─────────────────────────────────────────

    #[test]
    fn focused_border_color_follows_focus() {
        let bundle = bundle_with(vec![], None, vec![]);
        let border_fg = |focused: bool| {
            let mut state = AlertWorkbenchState::new();
            state.bottom_tab = AlertContextTab::Indicators;
            state.focused_pane = if focused {
                AlertPane::ContextPanel
            } else {
                AlertPane::AlertList
            };
            let backend = ratatui::backend::TestBackend::new(80, 14);
            let mut terminal = ratatui::Terminal::new(backend).unwrap();
            terminal
                .draw(|f| draw_context_panel(f, f.area(), Some(&bundle), &state, get_theme("dark")))
                .unwrap();
            terminal.backend().buffer()[(70, 0)].style().fg
        };
        let theme = get_theme("dark");
        assert_eq!(border_fg(true), Some(theme.primary));
        assert_eq!(border_fg(false), Some(theme.border));
    }

    #[test]
    fn does_not_panic_on_tiny_area() {
        let bundle = bundle_with(
            vec![indicator("x", IndicatorType::Cve, None)],
            Some("{}"),
            vec![],
        );
        for tab in AlertContextTab::ALL {
            let _ = render_text(Some(&bundle), &state_on_tab(tab), 12, 5);
        }
        let _ = render_text(None, &state_on_tab(AlertContextTab::Indicators), 8, 3);
    }

    // ── wrapped_line_count + scroll-clamp limits (#17) ─────────────────────

    #[test]
    fn wrapped_line_count_fits_one_row() {
        // A short line within `width` is one row.
        assert_eq!(wrapped_line_count("short", 80), 1);
        assert_eq!(
            wrapped_line_count(
                "a
b
c", 80
            ),
            3
        );
    }

    #[test]
    fn wrapped_line_count_wraps_long_lines() {
        // 20 chars in a 10-col column => 2 rows; two such lines => 4 rows.
        assert_eq!(wrapped_line_count(&"x".repeat(20), 10), 2);
        assert_eq!(
            wrapped_line_count(&format!("{}\n{}", "x".repeat(20), "y".repeat(20)), 10),
            4
        );
    }

    #[test]
    fn wrapped_line_count_empty_is_one_row() {
        assert_eq!(wrapped_line_count("", 80), 1);
    }

    #[test]
    fn metadata_wrap_scroll_reaches_long_tail() {
        // A single logical JSON line wrapping across many rows must be
        // scrollable to its tail (the clamp uses wrapped rows, not logical
        // lines, so max_scroll stays positive).
        let long = format!(r#"{{"long":"{}"}}"#, "z".repeat(2000));
        let bundle = bundle_with(vec![], Some(&long), vec![]);
        let mut state = state_on_tab(AlertContextTab::Metadata);

        // Top: the opening of the value is visible, the tail isn't.
        let top = render_text(Some(&bundle), &state, 30, 7);
        assert!(top.contains('z'), "value prefix should render\n{top}");
        assert!(!top.contains("TAIL"), "tail not yet rendered");

        // Mark the tail so we can detect it after scrolling.
        let long = format!(r#"{{"long":"{}TAIL"}}"#, "z".repeat(2000));
        let bundle = bundle_with(vec![], Some(&long), vec![]);
        // Scroll well past the top: the tail comes into view only because the
        // clamp uses the (large) wrapped row count, not 1 logical line.
        state.bottom_detail_scroll = 300;
        let scrolled = render_text(Some(&bundle), &state, 30, 7);
        assert!(
            scrolled.contains("TAIL"),
            "wrapped metadata tail must be scrollable into view:\n{scrolled}"
        );
    }
}

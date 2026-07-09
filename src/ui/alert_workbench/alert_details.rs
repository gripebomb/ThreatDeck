//! Top-right alert details pane renderer (Ticket 05).
//!
//! Renders an [`AlertDetailViewModel`] (loaded by the app service) inside the
//! layout engine's top-right rectangle. Pure presentation: no storage access.
//! Handles long snippets via wrapping + scroll, renders tags safely, shows
//! useful placeholders for missing optional values, and exposes loading and
//! no-selection states.
//!
//! See `tickets/05-alert-details-pane.md` and `docs/ARCHITECTURE.md`.

use chrono::{DateTime, Utc};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::theme::{criticality_color, Theme};
use crate::ui::alert_workbench::{AlertDetailViewModel, AlertPane, AlertWorkbenchState};
use crate::ui::list::criticality_label;

/// Render the top-right alert details pane into `area`.
///
/// Precedence: loading indicator > no-selection empty state > detail content.
/// Scrolling follows `state.right_detail_scroll`; the focused border follows
/// `state.focused_pane`.
pub fn draw_alert_details(
    f: &mut Frame,
    area: Rect,
    detail: Option<&AlertDetailViewModel>,
    state: &AlertWorkbenchState,
    theme: &Theme,
) {
    let focused = state.focused_pane == AlertPane::AlertDetails;
    let border_color = if focused { theme.primary } else { theme.border };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(" Alert Details ")
        .title_style(
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD),
        );

    if state.is_loading_details {
        let loading = Paragraph::new("Loading alert details…")
            .style(Style::default().fg(theme.muted))
            .alignment(Alignment::Center)
            .block(block);
        f.render_widget(loading, area);
        return;
    }

    let Some(detail) = detail else {
        let empty = Paragraph::new(
            "No alert selected.\n\nSelect an alert from the list to see its details.",
        )
        .style(Style::default().fg(theme.muted))
        .alignment(Alignment::Center)
        .block(block);
        f.render_widget(empty, area);
        return;
    };

    let lines = build_detail_lines(detail, theme);
    let content = Paragraph::new(lines)
        .style(Style::default().fg(theme.fg))
        .wrap(Wrap { trim: false })
        .scroll((state.right_detail_scroll, 0))
        .block(block);
    f.render_widget(content, area);
}

/// Compose the full detail view as styled lines.
fn build_detail_lines(detail: &AlertDetailViewModel, theme: &Theme) -> Vec<Line<'static>> {
    let muted = Style::default().fg(theme.muted);
    let mut lines: Vec<Line<'static>> = Vec::new();

    // ── Title ──────────────────────────────────────────────────────────────
    let title = detail
        .title
        .clone()
        .unwrap_or_else(|| "(untitled alert)".to_string());
    lines.push(Line::from(Span::styled(
        title,
        Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
    )));

    lines.push(Line::default());

    // ── Identity / triage fields ───────────────────────────────────────────
    lines.push(field("Feed", &detail.feed_name, theme));
    lines.push(field("Keyword", &detail.keyword_pattern, theme));

    // Severity: effective value (coloured) + base criticality for context.
    lines.push(Line::from(vec![
        Span::styled("Severity: ", muted),
        Span::styled(
            criticality_label(detail.severity),
            Style::default()
                .fg(criticality_color(theme, detail.severity))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  (base {})", criticality_label(detail.base_criticality)),
            muted,
        ),
    ]));

    lines.push(field("Status", &format!("{}", detail.status), theme));
    lines.push(field(
        "Disposition",
        &format!("{}", detail.disposition),
        theme,
    ));
    lines.push(field(
        "Confidence",
        &detail
            .confidence
            .map(|c| format!("{c}%"))
            .unwrap_or_else(|| "—".to_string()),
        theme,
    ));
    lines.push(field(
        "Owner",
        detail.owner.as_deref().unwrap_or("—"),
        theme,
    ));

    lines.push(Line::default());

    // ── Tags (rendered safely even when empty) ─────────────────────────────
    let tags_value = if detail.tags.is_empty() {
        "(none)".to_string()
    } else {
        detail.tags.join(", ")
    };
    lines.push(field("Tags", &tags_value, theme));

    lines.push(Line::default());

    // ── Snippet (wrapped, long content handled by Wrap) ────────────────────
    lines.push(Line::from(Span::styled("Snippet", muted)));
    lines.push(Line::from(detail.snippet.clone()));

    lines.push(Line::default());

    // ── Triage notes (only when present) ───────────────────────────────────
    if let Some(notes) = &detail.triage_notes {
        if !notes.trim().is_empty() {
            lines.push(Line::from(Span::styled("Triage notes", muted)));
            lines.push(Line::from(notes.clone()));
            lines.push(Line::default());
        }
    }

    // ── Timestamps ─────────────────────────────────────────────────────────
    lines.push(field("Detected", &fmt_time(detail.detected_at), theme));
    if let Some(ts) = detail.acknowledged_at {
        lines.push(field("Acknowledged", &fmt_time(ts), theme));
    }
    if let Some(ts) = detail.investigating_at {
        lines.push(field("Investigating", &fmt_time(ts), theme));
    }
    if let Some(ts) = detail.escalated_at {
        lines.push(field("Escalated", &fmt_time(ts), theme));
    }
    if let Some(ts) = detail.closed_at {
        let reason = detail.closed_reason.as_deref().unwrap_or("(no reason)");
        lines.push(field(
            "Closed",
            &format!("{} — {}", fmt_time(ts), reason),
            theme,
        ));
    }

    // ── Source URL (only when resolvable) ──────────────────────────────────
    if let Some(url) = &detail.source_url {
        if !url.is_empty() {
            lines.push(Line::default());
            lines.push(field("Source", url, theme));
        }
    }

    lines
}

/// A labelled field line: muted label + fg value.
fn field(label: &str, value: &str, theme: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label}: "), Style::default().fg(theme.muted)),
        Span::styled(value.to_string(), Style::default().fg(theme.fg)),
    ])
}

fn fmt_time(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

#[cfg(test)]
mod tests {
    //! TUI smoke tests for the alert details pane (Ticket 05).
    use super::*;
    use crate::theme::get_theme;
    use crate::types::{AlertDisposition, AlertStatus, Criticality};
    use chrono::Utc;
    use ratatui::{backend::TestBackend, Terminal};

    fn sample_detail() -> AlertDetailViewModel {
        AlertDetailViewModel {
            id: 42,
            title: Some("Ransomware campaign detected".to_string()),
            feed_name: "IOCFeed".to_string(),
            feed_url: Some("https://ioc.example.test/feed.xml".to_string()),
            keyword_pattern: "ransomware".to_string(),
            severity: Criticality::High,
            base_criticality: Criticality::High,
            status: AlertStatus::Investigating,
            disposition: AlertDisposition::ConfirmedThreat,
            confidence: Some(87),
            owner: Some("analyst1".to_string()),
            tags: vec!["apt".into(), "priority".into()],
            snippet: "Payload hash e3b0c44... posted at http://Bad.Example.NET/drop".to_string(),
            triage_notes: Some("Likely related to incident IR-22".to_string()),
            detected_at: Utc::now(),
            acknowledged_at: Some(Utc::now()),
            investigating_at: Some(Utc::now()),
            escalated_at: None,
            closed_at: None,
            closed_reason: None,
            source_url: None,
        }
    }

    fn render_text(
        detail: Option<&AlertDetailViewModel>,
        state: &AlertWorkbenchState,
        w: u16,
        h: u16,
    ) -> String {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw_alert_details(f, f.area(), detail, state, get_theme("dark")))
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

    fn focused_state() -> AlertWorkbenchState {
        let mut s = AlertWorkbenchState::new();
        s.focused_pane = AlertPane::AlertDetails;
        s
    }

    #[test]
    fn renders_all_detail_fields() {
        let detail = sample_detail();
        let text = render_text(Some(&detail), &focused_state(), 70, 30);
        assert!(
            text.contains("Ransomware campaign detected"),
            "title missing:\n{text}"
        );
        assert!(text.contains("Feed: IOCFeed"), "feed missing:\n{text}");
        assert!(text.contains("Keyword: ransomware"));
        assert!(text.contains("Severity:"), "severity missing:\n{text}");
        assert!(text.contains("High"), "severity label missing");
        assert!(text.contains("Status:"), "status missing");
        assert!(text.contains("Disposition:"));
        assert!(
            text.contains("Confidence: 87%"),
            "confidence missing:\n{text}"
        );
        assert!(text.contains("Owner: analyst1"));
        assert!(
            text.contains("Tags: apt, priority"),
            "tags missing:\n{text}"
        );
        assert!(
            text.contains("Payload hash e3b0c44"),
            "snippet missing:\n{text}"
        );
        assert!(text.contains("Triage notes"), "notes missing:\n{text}");
        assert!(text.contains("Detected:"), "detected missing:\n{text}");
    }

    #[test]
    fn handles_long_snippet_without_panicking() {
        let mut detail = sample_detail();
        detail.snippet = "x".repeat(2000);
        let text = render_text(Some(&detail), &focused_state(), 60, 24);
        // Must not panic; a prefix of the long snippet is visible.
        assert!(text.contains("xxxxx"));
    }

    #[test]
    fn missing_optional_values_show_placeholders() {
        let mut detail = sample_detail();
        detail.owner = None;
        detail.tags.clear();
        detail.triage_notes = None;
        detail.confidence = None;
        detail.source_url = None;
        let text = render_text(Some(&detail), &focused_state(), 70, 30);
        assert!(
            text.contains("Owner: —"),
            "owner placeholder missing:\n{text}"
        );
        assert!(
            text.contains("Confidence: —"),
            "confidence placeholder missing:\n{text}"
        );
        assert!(
            text.contains("Tags: (none)"),
            "tags placeholder missing:\n{text}"
        );
        // No notes section.
        assert!(!text.contains("Triage notes"));
    }

    #[test]
    fn shows_empty_state_when_no_selection() {
        let text = render_text(None, &focused_state(), 70, 16);
        assert!(
            text.contains("No alert selected"),
            "empty state missing:\n{text}"
        );
        assert!(text.contains("Alert Details"), "border title missing");
    }

    #[test]
    fn shows_loading_state_when_loading() {
        let mut state = focused_state();
        state.is_loading_details = true;
        let detail = sample_detail();
        let text = render_text(Some(&detail), &state, 70, 16);
        assert!(
            text.contains("Loading alert details"),
            "loading state missing:\n{text}"
        );
        // Should not render the detail content while loading.
        assert!(!text.contains("Ransomware campaign"));
    }

    #[test]
    fn scroll_offset_shifts_content() {
        let detail = sample_detail();
        // Without scroll the title is visible near the top.
        let top = render_text(Some(&detail), &focused_state(), 70, 10);
        assert!(top.contains("Ransomware campaign detected"));

        // With a large scroll the title scrolls out of view.
        let mut state = focused_state();
        state.right_detail_scroll = 200;
        let scrolled = render_text(Some(&detail), &state, 70, 10);
        assert!(
            !scrolled.contains("Ransomware campaign detected"),
            "scroll should push the title out:\n{scrolled}"
        );
    }

    #[test]
    fn focused_border_uses_primary_color() {
        let detail = sample_detail();

        let border_fg = |focused: bool| {
            let mut state = AlertWorkbenchState::new();
            state.focused_pane = if focused {
                AlertPane::AlertDetails
            } else {
                AlertPane::AlertList
            };
            let backend = TestBackend::new(70, 12);
            let mut terminal = Terminal::new(backend).unwrap();
            terminal
                .draw(|f| draw_alert_details(f, f.area(), Some(&detail), &state, get_theme("dark")))
                .unwrap();
            // Top border row at y=0; sample a cell past the title text.
            terminal.backend().buffer()[(60, 0)].style().fg
        };

        let theme = get_theme("dark");
        assert_eq!(border_fg(true), Some(theme.primary));
        assert_eq!(border_fg(false), Some(theme.border));
    }

    #[test]
    fn does_not_panic_on_tiny_area() {
        let detail = sample_detail();
        let _ = render_text(Some(&detail), &focused_state(), 10, 4);
        let _ = render_text(None, &focused_state(), 8, 3);
    }
}

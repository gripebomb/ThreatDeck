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
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::theme::{criticality_color, Theme};
use crate::ui::alert_workbench::{AlertDetailViewModel, AlertPane, AlertWorkbenchState};
use crate::ui::list::criticality_label;

/// Render the top-right alert details pane into `area`.
///
/// Precedence: error > no-selection empty state > detail content. Scrolling
/// follows `state.right_detail_scroll`; the focused border follows
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
        .title(if focused {
            " ◆ Alert Details "
        } else {
            " Alert Details "
        })
        .title_style(
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD),
        );

    // Precedence: error > no-selection > content.
    if let Some(err) = &state.last_error {
        let msg = format!("⚠ Couldn't load this alert:\n\n{err}");
        let error = Paragraph::new(msg)
            .style(Style::default().fg(theme.error))
            .alignment(Alignment::Center)
            .block(block);
        f.render_widget(error, area);
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

    let inner = block.inner(area);
    let width = inner.width.max(1) as usize;
    let lines = build_detail_lines(detail, width, theme);

    // Clamp the applied scroll so the view never runs past the content. The
    // stored offset may be larger; only the displayed offset is bounded.
    let content_lines = lines.len() as u16;
    let max_scroll = content_lines.saturating_sub(inner.height);
    let effective_scroll = state.right_detail_scroll.min(max_scroll);

    let content = Paragraph::new(lines)
        .style(Style::default().fg(theme.fg))
        .scroll((effective_scroll, 0))
        .block(block);
    f.render_widget(content, area);
}

/// Compose the full detail view as styled lines. `width` is used to pre-wrap
/// long free-text (title/snippet/notes) so the line count — and thus scroll
/// clamping — stays accurate.
fn build_detail_lines(
    detail: &AlertDetailViewModel,
    width: usize,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let muted = Style::default().fg(theme.muted);
    let mut lines: Vec<Line<'static>> = Vec::new();

    // ── Title ──────────────────────────────────────────────────────────────
    let title = detail
        .title
        .clone()
        .unwrap_or_else(|| "(untitled alert)".to_string());
    let title_style = Style::default().fg(theme.fg).add_modifier(Modifier::BOLD);
    for wrap in wrap_text(&title, width) {
        lines.push(Line::from(Span::styled(wrap, title_style)));
    }

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
    lines.extend(field_lines(
        "Owner",
        detail.owner.as_deref().unwrap_or("—"),
        width,
        theme,
    ));

    lines.push(Line::default());

    // ── Tags (rendered safely even when empty) ─────────────────────────────
    let tags_value = if detail.tags.is_empty() {
        "(none)".to_string()
    } else {
        detail.tags.join(", ")
    };
    lines.extend(field_lines("Tags", &tags_value, width, theme));

    lines.push(Line::default());

    // ── Snippet (pre-wrapped so scrolling clamps correctly) ────────────────
    if !detail.snippet.trim().is_empty() {
        lines.push(Line::from(Span::styled("Snippet", muted)));
        for wrap in wrap_text(&detail.snippet, width) {
            lines.push(Line::from(wrap));
        }
        lines.push(Line::default());
    }

    // ── Triage notes (only when present) ───────────────────────────────────
    if let Some(notes) = &detail.triage_notes {
        if !notes.trim().is_empty() {
            lines.push(Line::from(Span::styled("Triage notes", muted)));
            for wrap in wrap_text(notes, width) {
                lines.push(Line::from(wrap));
            }
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
            lines.extend(field_lines("Source", url, width, theme));
        }
    }

    lines
}

/// A labelled field that fits on one line (value known to be short: feed,
/// keyword, status, timestamps). Muted label + fg value.
fn field(label: &str, value: &str, theme: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label}: "), Style::default().fg(theme.muted)),
        Span::styled(value.to_string(), Style::default().fg(theme.fg)),
    ])
}

/// A labelled field whose value may exceed the pane width (URLs, tag lists,
/// owner emails). The value is wrapped to the space left after the label on
/// the first line, so every line fits `width` — no reliance on
/// `Paragraph::wrap`, keeping the rendered line count (and scroll clamping)
/// accurate. The label is muted on the first line.
fn field_lines(label: &str, value: &str, width: usize, theme: &Theme) -> Vec<Line<'static>> {
    let muted = Style::default().fg(theme.muted);
    let fg = Style::default().fg(theme.fg);
    let prefix = format!("{label}: ");
    // Wrap the value to the width remaining after the label on line 0.
    let budget = width.saturating_sub(str_width(&prefix)).max(1);
    wrap_text(value, budget)
        .into_iter()
        .enumerate()
        .map(|(i, chunk)| {
            if i == 0 {
                Line::from(vec![
                    Span::styled(prefix.clone(), muted),
                    Span::styled(chunk, fg),
                ])
            } else {
                Line::from(Span::styled(chunk, fg))
            }
        })
        .collect()
}

/// Display (cell) width of `s`, honouring wide characters. Uses ratatui's own
/// unicode-width accounting so no extra dependency is needed.
fn str_width(s: &str) -> usize {
    ratatui::text::Span::from(s).width()
}

fn fmt_time(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Word-wrap `text` to at most `width` display cells per line, preserving
/// explicit newlines as paragraph breaks. Used to make line counts (and thus
/// scroll clamping) accurate for long free-text.
///
/// Width accounting uses display (cell) width via [`str_width`], so wide
/// (e.g. CJK) characters count as 2 cells. Unbroken tokens longer than
/// `width` (hashes, URLs, base64 blobs) are hard-broken into `width`-sized
/// chunks so a single long word can't make a line taller than `width` cells.
/// Without this, `content_lines` undercounts the rendered height and
/// `max_scroll` clamps to 0, trapping long snippets out of view (see
/// `wraps_unbroken_long_word`).
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    let mut out = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        let mut len = 0usize;
        for word in paragraph.split_whitespace() {
            // Break an over-long token into `width`-cell chunks. The first
            // chunk may still join the current line; the rest each start a
            // fresh line.
            let mut chunks = split_long_word(word, width);
            let first = chunks.remove(0);
            let first_len = str_width(&first);
            let separator = if line.is_empty() { 0 } else { 1 };
            if !line.is_empty() && len + separator + first_len > width {
                out.push(std::mem::take(&mut line));
                len = 0;
            }
            if !line.is_empty() {
                line.push(' ');
                len += 1;
            }
            line.push_str(&first);
            len += first_len;
            // Remaining hard-broken chunks each occupy their own line.
            for chunk in chunks {
                out.push(std::mem::take(&mut line));
                line.push_str(&chunk);
                len = str_width(&chunk);
            }
        }
        out.push(line);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// Break `word` into chunks of at most `width` display cells. Short words
/// come back as a single chunk; over-long tokens are split so no chunk exceeds
/// `width` cells (chunking by cell width, not char count, so wide characters
/// aren't under-counted).
fn split_long_word(word: &str, width: usize) -> Vec<String> {
    if str_width(word) <= width {
        return vec![word.to_string()];
    }
    let mut chunks = Vec::new();
    let mut cur = String::new();
    let mut cur_w = 0usize;
    for ch in word.chars() {
        let cw = str_width(&ch.to_string());
        if cur_w + cw > width && !cur.is_empty() {
            chunks.push(std::mem::take(&mut cur));
            cur_w = 0;
        }
        cur.push(ch);
        cur_w += cw;
    }
    if !cur.is_empty() {
        chunks.push(cur);
    }
    chunks
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
    fn long_unbroken_snippet_is_scrollable_into_view() {
        // Regression for the reported blocker: an unbroken long token used to
        // make wrap_text emit a single over-long line, so `content_lines`
        // undercounted and `max_scroll` clamped to 0 — the tail of the snippet
        // could never be scrolled into view. Now the token is split and the
        // later portion becomes reachable by scrolling.
        let mut detail = sample_detail();
        // Mark the tail so we can detect it after scrolling.
        detail.snippet = format!("{}TAILMARKER", "x".repeat(2000));

        // At scroll 0 the tail marker is off-screen.
        let top = render_text(Some(&detail), &focused_state(), 40, 12);
        assert!(!top.contains("TAILMARKER"), "tail should be below the fold:\n{top}");

        // After scrolling well past the top, the tail marker comes into view.
        let mut state = focused_state();
        state.right_detail_scroll = 220;
        let scrolled = render_text(Some(&detail), &state, 40, 12);
        assert!(
            scrolled.contains("TAILMARKER"),
            "long snippet tail must be scrollable into view:\n{scrolled}"
        );
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
    fn shows_error_state_when_error_set() {
        let mut state = focused_state();
        state.last_error = Some("disk full".into());
        let detail = sample_detail();
        let text = render_text(Some(&detail), &state, 70, 16);
        assert!(
            text.contains("Couldn't load this alert"),
            "error state missing:\n{text}"
        );
        assert!(text.contains("disk full"), "error detail missing:\n{text}");
        // Detail content must not render while an error is shown.
        assert!(
            !text.contains("Feed:"),
            "content rendered during error:\n{text}"
        );
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

    // ── wrap_text: unbroken long words must be split (Issue #4) ───────────────
    #[test]
    fn wraps_unbroken_long_word() {
        // A single 2000-char token (hash/base64/URL) must be broken into
        // width-sized lines, none longer than `width`.
        let width = 40usize;
        let token = "A".repeat(2000);
        let lines = wrap_text(&token, width);
        assert!(lines.len() >= 50, "expected many lines, got {}", lines.len());
        for l in &lines {
            assert!(l.chars().count() <= width, "line exceeds width: {}", l.len());
        }
        // The whole token is preserved across the chunks.
        let joined: String = lines.join("");
        assert_eq!(joined, token);
    }

    #[test]
    fn wrap_text_mixed_words_and_long_token() {
        // Normal words wrap normally; an over-long URL is hard-broken.
        let width = 20usize;
        let text = "see https://example.com/very/long/path/that/keeps/going/and/going";
        let lines = wrap_text(text, width);
        for l in &lines {
            assert!(l.chars().count() <= width, "line exceeds width: {l}");
        }
        // The URL can't join "see" without overflowing width 20, so "see"
        // sits alone on the first line and the URL starts on the next.
        assert_eq!(lines[0], "see");
        assert!(lines[1].starts_with("https://"));
        // No information is lost: every character of the URL is present across
        // the wrapped chunks (the space before it is intentional word spacing).
        let url = "https://example.com/very/long/path/that/keeps/going/and/going";
        let url_joined: String = lines[1..].concat();
        assert_eq!(url_joined, url);
    }

    #[test]
    fn wrap_text_preserves_paragraph_breaks() {
        let lines = wrap_text("line one\nline two", 80);
        assert_eq!(lines, vec!["line one".to_string(), "line two".to_string()]);
    }

    #[test]
    fn wrap_text_empty_returns_single_empty_line() {
        assert_eq!(wrap_text("", 10), vec!["".to_string()]);
    }
}

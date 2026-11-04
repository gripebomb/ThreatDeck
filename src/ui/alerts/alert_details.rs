use crate::app::App;
use crate::ui::list::criticality_label;
use ratatui::{
    layout::Rect,
    style::Style,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

/// Render the top-right alert details pane from the loaded
/// `AlertWorkbenchBundle`.
///
/// `focused` toggles the border accent: `theme.primary` when the pane holds
/// focus, `theme.border` otherwise. When no bundle is loaded (nothing
/// selected / still loading), a muted prompt is shown instead.
pub fn draw_alert_details(f: &mut Frame, app: &App, area: Rect, focused: bool) {
    let border_color = if focused {
        app.theme.primary
    } else {
        app.theme.border
    };
    let focus_marker = if focused { "▶ " } else { "  " };
    let hints = if focused {
        " · j/k scroll · x export"
    } else {
        ""
    };
    let block = Block::default()
        .title(format!("{}Alert Details{}", focus_marker, hints))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    // Empty / loading state: nothing selected yet.
    if app.workbench_bundle.is_none() {
        let para = Paragraph::new("Select an alert to view details.")
            .style(Style::default().fg(app.theme.muted))
            .block(block);
        f.render_widget(para, area);
        return;
    }

    let bundle = app.workbench_bundle.as_ref().unwrap();
    let d = &bundle.detail;

    let tags = if d.tags.is_empty() {
        "(none)".to_string()
    } else {
        d.tags
            .iter()
            .map(|t| t.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    };

    let mut lines: Vec<String> = vec![
        format!("Title: {}", d.title),
        format!("Feed: {}", d.feed_name),
        format!("Keyword: {}", d.keyword_pattern),
        format!("Severity: {}", criticality_label(d.effective_severity)),
        format!("Status: {}", d.status),
        format!("Disposition: {}", d.disposition),
        format!(
            "Confidence: {}",
            d.confidence_score
                .map(|c| c.to_string())
                .unwrap_or_else(|| "-".into())
        ),
        format!("Owner: {}", d.owner.as_deref().unwrap_or("-")),
        format!("Tags: {}", tags),
        String::new(),
        format!("Detected: {}", d.detected_at.format("%Y-%m-%d %H:%M:%S")),
    ];

    // Lifecycle timestamps — only the ones that are set.
    if let Some(ts) = d.acknowledged_at {
        lines.push(format!("Acknowledged: {}", ts.format("%Y-%m-%d %H:%M")));
    }
    if let Some(ts) = d.investigating_at {
        lines.push(format!("Investigating: {}", ts.format("%Y-%m-%d %H:%M")));
    }
    if let Some(ts) = d.escalated_at {
        lines.push(format!("Escalated: {}", ts.format("%Y-%m-%d %H:%M")));
    }
    if let Some(ts) = d.closed_at {
        lines.push(format!(
            "Closed: {} — {}",
            ts.format("%Y-%m-%d %H:%M"),
            d.closed_reason.as_deref().unwrap_or("(no reason)")
        ));
    }

    lines.push(String::new());
    lines.push(format!(
        "Notes: {}",
        d.triage_notes.as_deref().unwrap_or("(none)")
    ));
    lines.push(String::new());
    lines.push(d.content_snippet.clone());

    let para = Paragraph::new(lines.join("\n"))
        .style(Style::default().fg(app.theme.fg))
        .wrap(Wrap { trim: false })
        .scroll((app.workbench_state.detail_scroll, 0))
        .block(block);
    f.render_widget(para, area);
}

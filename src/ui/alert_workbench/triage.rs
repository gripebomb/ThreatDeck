//! Triage & export actions for the alert workbench (Ticket 09).
//!
//! Unlike the legacy alerts screen (which keyed off `alerts_list` /
//! `alerts_selected`), these operate on the workbench's selected alert
//! (`app.workbench.selected_alert_id()`). After each mutation we reload the
//! selected bundle and/or the list so the panes reflect the change, and surface
//! the result via the app notification bar.
//!
//! Input modes:
//! - `triage_enum_select_mode` — pick status / disposition / severity from a
//!   numbered popup (`s` / `T` / `S`). Runs under `InputMode::Typing` so the
//!   digit keys (which are otherwise global screen-switch hotkeys) reach us.
//! - `triage_note_input_mode` — free-text entry for a triage note (`N`) or the
//!   alert owner (`M`).
//!
//! Esc is handled centrally in `App::handle_key` (the `Typing`-Esc branch
//! cancels either input mode), so these handlers never see Esc in practice —
//! the arms remain for defence.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::App;
use crate::types::{
    AlertDisposition, AlertStatus, Criticality, NotificationType, TriageEnumTarget,
    TriageInputTarget,
};

// ── Input-mode routing ───────────────────────────────────────────────────

/// Route a key to the active triage input mode, if any. Returns true if
/// consumed. Call this at the top of the page key handler.
pub fn handle_input_modes(app: &mut App, key: KeyEvent) -> bool {
    if app.triage_enum_select_mode {
        handle_enum_select_key(app, key);
        true
    } else if app.triage_note_input_mode {
        handle_note_input_key(app, key);
        true
    } else {
        false
    }
}

// ── Status actions ───────────────────────────────────────────────────────

/// Acknowledge the selected alert (`A`).
pub fn acknowledge(app: &mut App) {
    set_status(
        app,
        AlertStatus::Acknowledged,
        "Alert acknowledged",
        NotificationType::Success,
    );
}

/// Start investigating the selected alert (`I`).
pub fn investigate(app: &mut App) {
    set_status(
        app,
        AlertStatus::Investigating,
        "Investigation started",
        NotificationType::Success,
    );
}

/// Escalate the selected alert (`E`).
pub fn escalate(app: &mut App) {
    set_status(
        app,
        AlertStatus::Escalated,
        "Alert escalated",
        NotificationType::Warning,
    );
}

/// Reopen the selected alert (`O`). Only meaningful for a `Closed` alert;
/// reopening an already-open alert would silently re-acknowledge it with a
/// misleading success message, so we refuse with a warning instead.
pub fn reopen(app: &mut App) {
    let Some(id) = app.workbench.selected_alert_id() else {
        return;
    };
    let is_closed = app
        .workbench_bundle
        .as_ref()
        .and_then(|b| b.detail.as_ref())
        .map(|d| d.status == AlertStatus::Closed)
        .unwrap_or(false);
    if !is_closed {
        app.set_notification("Alert is not closed".into(), NotificationType::Warning);
        return;
    }
    match app.db.reopen_alert(id, None) {
        Ok(()) => {
            app.refresh_workbench();
            app.set_notification("Alert reopened".into(), NotificationType::Success);
        }
        Err(e) => app.set_notification(
            format!("Failed to reopen alert: {e}"),
            NotificationType::Error,
        ),
    }
}

/// Close the selected alert (`C`). Requires a non-`Unknown` disposition; if
/// none is set, prompt the user to pick one (`T`).
pub fn close(app: &mut App) {
    let Some(id) = app.workbench.selected_alert_id() else {
        return;
    };
    let disposition = app
        .workbench_bundle
        .as_ref()
        .and_then(|b| b.detail.as_ref())
        .map(|d| d.disposition);
    match disposition {
        Some(d) if d != AlertDisposition::Unknown => {
            let res = app
                .db
                .close_alert(id, d, None)
                .and_then(|()| app.db.mark_alert_read(id, true));
            // Refresh so the panes reflect whatever actually persisted.
            app.refresh_workbench();
            match res {
                Ok(()) => app.set_notification("Alert closed".into(), NotificationType::Success),
                Err(e) => app.set_notification(
                    format!("Failed to close alert: {e}"),
                    NotificationType::Error,
                ),
            }
        }
        _ => app.set_notification(
            "Set a disposition first (T)".into(),
            NotificationType::Warning,
        ),
    }
}

fn set_status(app: &mut App, status: AlertStatus, msg: &str, typ: NotificationType) {
    let Some(id) = app.workbench.selected_alert_id() else {
        return;
    };
    let res = app
        .db
        .update_alert_status(id, status, None)
        .and_then(|()| app.db.mark_alert_read(id, true));
    // Refresh so the panes reflect whatever actually persisted.
    app.refresh_workbench();
    match res {
        Ok(()) => app.set_notification(msg.into(), typ),
        Err(e) => app.set_notification(
            format!("Failed to update alert: {e}"),
            NotificationType::Error,
        ),
    }
}

// ── Enum selectors (status / disposition / severity) ────────────────────

/// Open the enum selector for the given target (`s` / `T` / `S`).
pub fn start_enum_select(app: &mut App, target: TriageEnumTarget) {
    if app.workbench.selected_alert_id().is_none() {
        return;
    }
    app.triage_enum_select_mode = true;
    app.triage_enum_target = Some(target);
    app.triage_enum_selected = 0;
    // Typing mode routes the digit keys (otherwise global screen switches) here.
    app.input_mode = crate::app::InputMode::Typing;
}

fn handle_enum_select_key(app: &mut App, key: KeyEvent) {
    let target = app.triage_enum_target.unwrap_or(TriageEnumTarget::Status);
    match key.code {
        KeyCode::Esc | KeyCode::Char('0') => {
            cancel_enum(app);
        }
        KeyCode::Char(c) if c.is_ascii_digit() => {
            let idx = (c as usize).saturating_sub('0' as usize).saturating_sub(1);
            if let Some(id) = app.workbench.selected_alert_id() {
                match target {
                    TriageEnumTarget::Status => {
                        if let Some((s, label)) = status_options().get(idx).copied() {
                            let res = app
                                .db
                                .update_alert_status(id, s, None)
                                .and_then(|()| app.db.mark_alert_read(id, true));
                            finish_enum(app, res, format!("Status: {label}"));
                        }
                    }
                    TriageEnumTarget::Disposition => {
                        if let Some((d, label)) = disposition_options().get(idx).copied() {
                            let res = app.db.update_alert_disposition(id, d, None);
                            finish_enum(app, res, format!("Disposition: {label}"));
                        }
                    }
                    TriageEnumTarget::Severity => {
                        if let Some((sev, label)) = severity_options().get(idx).copied() {
                            let res = app.db.update_alert_severity(id, Some(sev), None);
                            finish_enum(app, res, format!("Severity: {label}"));
                        }
                    }
                }
            }
            cancel_enum(app);
        }
        _ => {}
    }
}

fn finish_enum(app: &mut App, res: anyhow::Result<()>, msg: String) {
    app.refresh_workbench();
    match res {
        Ok(()) => app.set_notification(msg, NotificationType::Success),
        Err(e) => app.set_notification(
            format!("Failed to update alert: {e}"),
            NotificationType::Error,
        ),
    }
}

fn cancel_enum(app: &mut App) {
    app.triage_enum_select_mode = false;
    app.triage_enum_target = None;
    app.triage_enum_selected = 0;
    app.input_mode = crate::app::InputMode::Normal;
}

fn status_options() -> Vec<(AlertStatus, &'static str)> {
    vec![
        (AlertStatus::New, "New"),
        (AlertStatus::Acknowledged, "Acknowledged"),
        (AlertStatus::Investigating, "Investigating"),
        (AlertStatus::Escalated, "Escalated"),
        (AlertStatus::Closed, "Closed"),
    ]
}

fn disposition_options() -> Vec<(AlertDisposition, &'static str)> {
    vec![
        (AlertDisposition::Unknown, "Unknown"),
        (AlertDisposition::ConfirmedThreat, "Confirmed threat"),
        (AlertDisposition::FalsePositive, "False positive"),
        (AlertDisposition::Benign, "Benign"),
        (AlertDisposition::Duplicate, "Duplicate"),
        (AlertDisposition::Informational, "Informational"),
        (AlertDisposition::NeedsMoreContext, "Needs more context"),
    ]
}

fn severity_options() -> Vec<(Criticality, &'static str)> {
    vec![
        (Criticality::Low, "Low"),
        (Criticality::Medium, "Medium"),
        (Criticality::High, "High"),
        (Criticality::Critical, "Critical"),
    ]
}

// ── Note / owner entry ──────────────────────────────────────────────────

/// Begin free-text entry for a triage note (`N`) or the owner (`M`).
pub fn start_note_input(app: &mut App, target: TriageInputTarget) {
    if app.workbench.selected_alert_id().is_none() {
        return;
    }
    app.triage_note_input_mode = true;
    app.triage_note_input.clear();
    app.triage_input_target = target;
    app.input_mode = crate::app::InputMode::Typing;
}

fn handle_note_input_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => cancel_note(app),
        KeyCode::Enter => {
            let text = app.triage_note_input.trim().to_string();
            if !text.is_empty() {
                if let Some(id) = app.workbench.selected_alert_id() {
                    let (res, ok_msg): (anyhow::Result<()>, String) = match app.triage_input_target
                    {
                        TriageInputTarget::Note => {
                            (app.db.add_alert_note(id, &text), "Note saved".to_string())
                        }
                        TriageInputTarget::Owner => (
                            app.db.assign_alert_owner(id, Some(&text), None),
                            format!("Owner set to {text}"),
                        ),
                    };
                    // Note/owner don't affect list ordering/filter, so just
                    // refresh the bundle (keeps the detail scroll position).
                    app.refresh_workbench_bundle();
                    match res {
                        Ok(()) => app.set_notification(ok_msg, NotificationType::Success),
                        Err(e) => app.set_notification(
                            format!("Failed to save: {e}"),
                            NotificationType::Error,
                        ),
                    }
                }
            }
            cancel_note(app);
        }
        KeyCode::Backspace => {
            app.triage_note_input.pop();
        }
        KeyCode::Char(c) => {
            app.triage_note_input.push(c);
        }
        _ => {}
    }
}

fn cancel_note(app: &mut App) {
    app.triage_note_input_mode = false;
    app.triage_note_input.clear();
    app.triage_input_target = TriageInputTarget::default();
    app.input_mode = crate::app::InputMode::Normal;
}

// ── Markdown export ─────────────────────────────────────────────────────

/// Export the selected alert to a Markdown report under `data_dir/exports` (`x`).
pub fn export_selected(app: &mut App) {
    let Some(id) = app.workbench.selected_alert_id() else {
        return;
    };
    let options = threatdeck_report::ReportExportOptions {
        report_type: threatdeck_report::ReportType::Alert,
        format: threatdeck_report::ExportFormat::Markdown,
        output_path: None,
        include_raw_content: false,
        include_metadata: true,
        include_iocs: true,
        include_enrichment: true,
        include_triage_history: true,
        include_feed_health: false,
        include_tags: true,
        redact_secrets: true,
        overwrite: false,
        generated_by: None,
    };
    let export_dir = app.paths.data_dir.join("exports");
    let report_service = crate::report::ReportService::new();
    match report_service.export_alert_report(&app.db, id, &options, &export_dir) {
        Ok(result) => app.set_notification(
            format!("Exported: {}", result.path.display()),
            NotificationType::Success,
        ),
        Err(e) => app.set_notification(format!("Export failed: {e}"), NotificationType::Error),
    }
}

// ── Overlays ────────────────────────────────────────────────────────────

/// Render any active triage input popups on top of the workbench.
pub fn draw_overlays(f: &mut Frame, app: &App) {
    if app.triage_enum_select_mode {
        draw_enum_selector(f, app);
    }
    if app.triage_note_input_mode {
        draw_note_input(f, app);
    }
}

fn draw_enum_selector(f: &mut Frame, app: &App) {
    let target = app.triage_enum_target.unwrap_or(TriageEnumTarget::Status);
    let (title, options): (&'static str, Vec<&'static str>) = match target {
        TriageEnumTarget::Status => (
            "Set Status",
            status_options().iter().map(|(_, l)| *l).collect(),
        ),
        TriageEnumTarget::Disposition => (
            "Set Disposition",
            disposition_options().iter().map(|(_, l)| *l).collect(),
        ),
        TriageEnumTarget::Severity => (
            "Set Severity",
            severity_options().iter().map(|(_, l)| *l).collect(),
        ),
    };

    let muted = Style::default().fg(app.theme.muted);
    let mut lines: Vec<Line> = Vec::with_capacity(options.len() + 2);
    lines.push(Line::from(Span::styled("Pick an option:", muted)));
    for (i, label) in options.iter().enumerate() {
        lines.push(Line::from(format!("  {}  {label}", i + 1)));
    }
    lines.push(Line::from(Span::styled("[0/Esc] Cancel", muted)));

    let height = options.len() as u16 + 4;
    let area = centered(f.area(), 44, height);
    f.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {title} "))
        .title_style(
            Style::default()
                .fg(app.theme.primary)
                .add_modifier(Modifier::BOLD),
        )
        .border_style(Style::default().fg(app.theme.primary));
    f.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(app.theme.fg))
            .block(block),
        area,
    );
}

fn draw_note_input(f: &mut Frame, app: &App) {
    let (title, prompt) = match app.triage_input_target {
        TriageInputTarget::Note => ("Add Note", "Note:"),
        TriageInputTarget::Owner => ("Set Owner", "Owner:"),
    };
    let muted = Style::default().fg(app.theme.muted);
    let lines = vec![
        Line::from(Span::styled(prompt, muted)),
        Line::from(app.triage_note_input.clone()),
        Line::from(Span::styled("[Enter] Save   [Esc] Cancel", muted)),
    ];
    let area = centered(f.area(), 52, 5);
    f.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {title} "))
        .title_style(
            Style::default()
                .fg(app.theme.primary)
                .add_modifier(Modifier::BOLD),
        )
        .border_style(Style::default().fg(app.theme.primary));
    f.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(app.theme.fg))
            .block(block),
        area,
    );
}

/// Centre a `width`×`height` rect within `r`, clamped to fit.
fn centered(r: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(r.width);
    let h = height.min(r.height);
    Rect {
        x: r.x + (r.width - w) / 2,
        y: r.y + (r.height - h) / 2,
        width: w,
        height: h,
    }
}

#[cfg(test)]
mod tests {
    //! Triage action integration tests (Ticket 09).
    //!
    //! These seed a temp DB, drive the triage functions through a real `App`,
    //! and assert the storage + view-model layer reflects the change.
    use super::*;
    use crate::config::{AppConfig, Paths};
    use crate::db::{AlertCreate, Db, FeedCreate, KeywordCreate};
    use crate::types::{Criticality, FeedType, Screen};
    use std::path::PathBuf;

    fn temp_db(name: &str) -> (Db, PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "threatdeck-wb-triage-{}-{}.db",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let db = Db::open(&path).unwrap();
        db.init_schema().unwrap();
        (db, path)
    }

    fn seed_one(db: &Db) -> i64 {
        let feed_id = db
            .create_feed(&FeedCreate {
                name: "TriageFeed".into(),
                url: "https://triage.example.test/feed.xml".into(),
                feed_type: FeedType::Rss,
                enabled: true,
                interval_secs: 300,
                ..FeedCreate::default()
            })
            .unwrap();
        let keyword_id = db
            .create_keyword(&KeywordCreate {
                pattern: "ransomware".into(),
                ..KeywordCreate::default()
            })
            .unwrap();
        db.create_alert(&AlertCreate {
            feed_id,
            keyword_id,
            title: Some("TriageTarget".into()),
            content_snippet: "snippet".into(),
            content_hash: "triage-hash-1".into(),
            criticality: Criticality::High,
            metadata_json: None,
        })
        .unwrap()
    }

    fn build_app(name: &str) -> (App, PathBuf) {
        let (db, path) = temp_db(name);
        let id = seed_one(&db);
        let paths = Paths {
            config_dir: PathBuf::new(),
            data_dir: std::env::temp_dir(),
            config_file: PathBuf::new(),
            db_file: path.clone(),
        };
        let mut app = App::new(db, AppConfig::default(), paths);
        app.screen = Screen::Alerts;
        // Filter to the seeded alert (init_schema also seeds a demo catalog).
        app.alerts_filter = "TriageTarget".to_string();
        // Keep closed alerts visible so close/reopen flows can be inspected.
        app.alerts_hide_closed = false;
        app.refresh_workbench();
        assert_eq!(app.workbench.selected_alert_id(), Some(id));
        (app, path)
    }

    fn selected_status(app: &App) -> crate::types::AlertStatus {
        app.workbench_bundle
            .as_ref()
            .and_then(|b| b.detail.as_ref())
            .expect("detail loaded")
            .status
    }

    #[test]
    fn acknowledge_sets_status_and_marks_read() {
        let (mut app, path) = build_app("ack");
        acknowledge(&mut app);
        assert_eq!(
            selected_status(&app),
            crate::types::AlertStatus::Acknowledged
        );
        assert_eq!(
            app.notification.as_ref().unwrap().1,
            NotificationType::Success
        );
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn escalate_then_investigate_update_status() {
        let (mut app, path) = build_app("esc");
        escalate(&mut app);
        assert_eq!(selected_status(&app), crate::types::AlertStatus::Escalated);
        investigate(&mut app);
        assert_eq!(
            selected_status(&app),
            crate::types::AlertStatus::Investigating
        );
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn close_requires_disposition_first() {
        let (mut app, path) = build_app("closenodisp");
        // Default disposition is Unknown → close should refuse with a warning.
        close(&mut app);
        assert_eq!(selected_status(&app), crate::types::AlertStatus::New);
        assert_eq!(
            app.notification.as_ref().unwrap().1,
            NotificationType::Warning
        );

        // Set a disposition via the selector, then close succeeds.
        start_enum_select(&mut app, TriageEnumTarget::Disposition);
        handle_enum_select_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('2'), crossterm::event::KeyModifiers::NONE),
        ); // ConfirmedThreat
        assert_eq!(
            app.workbench_bundle
                .as_ref()
                .and_then(|b| b.detail.as_ref())
                .unwrap()
                .disposition,
            AlertDisposition::ConfirmedThreat,
        );
        close(&mut app);
        assert_eq!(selected_status(&app), crate::types::AlertStatus::Closed);
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reopen_restores_open_status() {
        let (mut app, path) = build_app("reopen");
        // Close via disposition + close.
        start_enum_select(&mut app, TriageEnumTarget::Disposition);
        handle_enum_select_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('2'), crossterm::event::KeyModifiers::NONE),
        );
        close(&mut app);
        assert_eq!(selected_status(&app), crate::types::AlertStatus::Closed);
        // Reopen → no longer Closed.
        reopen(&mut app);
        assert_ne!(selected_status(&app), crate::types::AlertStatus::Closed);
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reopen_refuses_non_closed_alert() {
        let (mut app, path) = build_app("reopenopen");
        // The seeded alert starts as New (not Closed).
        assert_eq!(selected_status(&app), crate::types::AlertStatus::New);
        reopen(&mut app);
        // No mutation happened: still New, and a warning was shown.
        assert_eq!(selected_status(&app), crate::types::AlertStatus::New);
        assert_eq!(
            app.notification.as_ref().unwrap().1,
            NotificationType::Warning
        );
        assert_eq!(app.notification.as_ref().unwrap().0, "Alert is not closed");
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn note_input_saves_and_owner_input_assigns() {
        let (mut app, path) = build_app("note");

        // Owner entry.
        start_note_input(&mut app, TriageInputTarget::Owner);
        for ch in "analyst-1".chars() {
            handle_note_input_key(
                &mut app,
                KeyEvent::new(KeyCode::Char(ch), crossterm::event::KeyModifiers::NONE),
            );
        }
        handle_note_input_key(
            &mut app,
            KeyEvent::new(KeyCode::Enter, crossterm::event::KeyModifiers::NONE),
        );
        assert_eq!(
            app.workbench_bundle
                .as_ref()
                .and_then(|b| b.detail.as_ref())
                .unwrap()
                .owner
                .as_deref(),
            Some("analyst-1"),
        );

        // Note entry.
        start_note_input(&mut app, TriageInputTarget::Note);
        for ch in "looks malicious".chars() {
            handle_note_input_key(
                &mut app,
                KeyEvent::new(KeyCode::Char(ch), crossterm::event::KeyModifiers::NONE),
            );
        }
        handle_note_input_key(
            &mut app,
            KeyEvent::new(KeyCode::Enter, crossterm::event::KeyModifiers::NONE),
        );
        assert_eq!(
            app.workbench_bundle
                .as_ref()
                .and_then(|b| b.detail.as_ref())
                .unwrap()
                .triage_notes
                .as_deref(),
            Some("looks malicious"),
        );
        // Modes cleared after save.
        assert!(!app.triage_note_input_mode);
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn severity_selector_updates_severity() {
        let (mut app, path) = build_app("sev");
        start_enum_select(&mut app, TriageEnumTarget::Severity);
        handle_enum_select_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('4'), crossterm::event::KeyModifiers::NONE),
        ); // Critical
        assert_eq!(
            app.workbench_bundle
                .as_ref()
                .and_then(|b| b.detail.as_ref())
                .unwrap()
                .severity,
            Criticality::Critical,
        );
        assert!(
            !app.triage_enum_select_mode,
            "enum mode should clear after pick"
        );
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn actions_without_selection_are_noops() {
        let (mut app, path) = build_app("noop");
        // Clear the selection by filtering to nothing.
        app.alerts_filter = "zzzz-none".into();
        app.refresh_workbench();
        assert!(app.workbench.selected_alert_id().is_none());

        acknowledge(&mut app);
        assert!(
            app.notification.is_none(),
            "no notification without selection"
        );
        start_enum_select(&mut app, TriageEnumTarget::Status);
        assert!(!app.triage_enum_select_mode, "enum select should not open");
        start_note_input(&mut app, TriageInputTarget::Note);
        assert!(!app.triage_note_input_mode, "note input should not open");
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn export_selected_writes_markdown_file() {
        let (mut app, path) = build_app("export");
        export_selected(&mut app);
        let notif = app.notification.as_ref().expect("notification set");
        assert_eq!(notif.1, NotificationType::Success);
        // The notification message contains the export path; the file exists.
        let msg = &notif.0;
        assert!(msg.contains("Exported:"), "{msg}");
        let exported = msg.trim_start_matches("Exported: ").trim();
        assert!(
            std::path::Path::new(exported).exists(),
            "export file missing: {exported}"
        );
        drop(app);
        let _ = std::fs::remove_file(path);
    }
}

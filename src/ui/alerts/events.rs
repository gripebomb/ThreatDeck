//! Keyboard event handling for the split-pane alert workbench.
//!
//! Dispatches keys based on the focused pane:
//! - List pane: alert selection (j/k/g/G/arrows), Enter to focus details
//! - Details pane: vertical scroll (j/k/arrows)
//! - Context pane: tab switching ([/]), vertical scroll (j/k/arrows)
//!
//! Global keys (Tab/BackTab for pane cycling, r for refresh, triage actions)
//! work regardless of focused pane.

use crate::app::App;
use crate::types::{Criticality, NotificationType, TriageEnumTarget};
use crate::ui::alerts::state::{AlertContextTab, AlertPane};
use crate::ui::list::{motion_from_key, move_selection};
use crossterm::event::{KeyCode, KeyEvent};

/// Handle a key in workbench (normal) mode.
pub fn handle_workbench_key(app: &mut App, key: KeyEvent) {
    // ── Global keys (any pane) ──────────────────────────────────────────────
    match key.code {
        KeyCode::Tab => {
            app.workbench_state.focus_next_pane();
            return;
        }
        KeyCode::BackTab => {
            app.workbench_state.focus_prev_pane();
            return;
        }
        KeyCode::Char('r') => {
            app.alerts_selected = app.workbench_state.selected_alert_index;
            app.refresh_alerts();
            app.refresh_dashboard();
            return;
        }
        // Triage actions (operate on selected alert from any pane)
        KeyCode::Char('A') => {
            app.triage_acknowledge();
            return;
        }
        KeyCode::Char('I') => {
            app.triage_investigate();
            return;
        }
        KeyCode::Char('E') => {
            app.triage_escalate();
            return;
        }
        KeyCode::Char('O') => {
            app.triage_reopen();
            return;
        }
        KeyCode::Char('C') => {
            app.triage_close();
            return;
        }
        KeyCode::Char('N') => {
            app.triage_note_input_mode = true;
            app.triage_note_input.clear();
            app.triage_input_target = crate::types::TriageInputTarget::Note;
            app.input_mode = crate::app::InputMode::Typing;
            return;
        }
        KeyCode::Char('M') => {
            app.triage_note_input_mode = true;
            app.triage_note_input.clear();
            app.triage_input_target = crate::types::TriageInputTarget::Owner;
            app.input_mode = crate::app::InputMode::Typing;
            return;
        }
        KeyCode::Char('s') => {
            app.triage_enum_select_mode = true;
            app.triage_enum_target = Some(TriageEnumTarget::Status);
            return;
        }
        KeyCode::Char('S') => {
            app.triage_enum_select_mode = true;
            app.triage_enum_target = Some(TriageEnumTarget::Severity);
            return;
        }
        KeyCode::Char('T') => {
            app.triage_enum_select_mode = true;
            app.triage_enum_target = Some(TriageEnumTarget::Disposition);
            return;
        }
        KeyCode::Char('H') => {
            app.triage_history_view = true;
            return;
        }
        KeyCode::Char('R') => {
            let _ = app.db.mark_all_alerts_read(true);
            app.refresh_alerts();
            app.refresh_dashboard();
            app.set_notification(
                "All alerts marked as read".to_string(),
                NotificationType::Success,
            );
            return;
        }
        KeyCode::Char('d') => {
            if let Some(a) = app.alerts_list.get(app.alerts_selected) {
                app.show_confirm =
                    Some(crate::types::ConfirmDialog::DeleteAlert { id: a.alert.id });
            }
            return;
        }
        KeyCode::Char('b') => {
            app.alerts_bulk_mode = true;
            app.alerts_selected_bulk.clear();
            return;
        }
        KeyCode::Char('t') => {
            if let Some(a) = app.alerts_list.get(app.alerts_selected) {
                app.tags_assignment_mode = true;
                app.tags_assignment_target =
                    Some(crate::types::TagAssignmentTarget::Alert(a.alert.id));
                app.refresh_tags();
            }
            return;
        }
        KeyCode::Char('h') => {
            app.alerts_hide_closed = !app.alerts_hide_closed;
            app.refresh_alerts();
            app.set_notification(
                if app.alerts_hide_closed {
                    "Hiding closed alerts"
                } else {
                    "Showing all alerts"
                }
                .into(),
                NotificationType::Info,
            );
            return;
        }
        KeyCode::Char('c') => {
            app.alerts_filter_criticality = match app.alerts_filter_criticality {
                None => Some(Criticality::Low),
                Some(Criticality::Low) => Some(Criticality::Medium),
                Some(Criticality::Medium) => Some(Criticality::High),
                Some(Criticality::High) => Some(Criticality::Critical),
                Some(Criticality::Critical) => None,
            };
            app.refresh_alerts();
            return;
        }
        KeyCode::Char('x') => {
            app.export_selected_alert_markdown();
            return;
        }
        _ => {}
    }

    // ── Pane-specific keys ──────────────────────────────────────────────────
    match app.workbench_state.focused_pane {
        AlertPane::List => handle_list_pane_key(app, key),
        AlertPane::Details => handle_scroll_pane_key(app, key),
        AlertPane::Context => handle_context_pane_key(app, key),
    }
}

/// Handle keys when the alert list pane is focused.
fn handle_list_pane_key(app: &mut App, key: KeyEvent) {
    if let Some(motion) = motion_from_key(key, &mut app.pending_g) {
        let len = app.workbench_state.alerts.len();
        let new_idx = move_selection(app.workbench_state.selected_alert_index, len, motion);
        if new_idx != app.workbench_state.selected_alert_index {
            app.workbench_state.selected_alert_index = new_idx;
            app.workbench_state.detail_scroll = 0;
            app.workbench_state.context_scroll = 0;
            app.workbench_state.reset_ioc_selection();
            app.refresh_workbench_bundle();
        }
        return;
    }

    match key.code {
        KeyCode::Enter => {
            app.workbench_state.focus_pane(AlertPane::Details);
        }
        KeyCode::Char(' ') => {
            if let Some(a) = app.alerts_list.get(app.alerts_selected) {
                let new_read = !a.alert.read;
                let _ = app.db.mark_alert_read(a.alert.id, new_read);
                app.refresh_alerts();
                app.refresh_dashboard();
            }
        }
        _ => {}
    }
}

/// Handle scroll keys for the details pane.
fn handle_scroll_pane_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            app.workbench_state.detail_scroll = app.workbench_state.detail_scroll.saturating_add(1);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.workbench_state.detail_scroll = app.workbench_state.detail_scroll.saturating_sub(1);
        }
        _ => {}
    }
}

/// Handle keys for the context panel.
///
/// IOC tab: j/k moves IOC selection, y copies IOC, e enriches IOC.
/// Other tabs: j/k scrolls content.
/// All tabs: [/] switches tabs.
fn handle_context_pane_key(app: &mut App, key: KeyEvent) {
    // Tab switching works on any tab
    match key.code {
        KeyCode::Char('[') => {
            app.workbench_state.cycle_context_tab_prev();
            app.workbench_state.context_scroll = 0;
            return;
        }
        KeyCode::Char(']') => {
            app.workbench_state.cycle_context_tab_next();
            app.workbench_state.context_scroll = 0;
            return;
        }
        _ => {}
    }

    // IOC-tab-specific keys: selection movement, copy, enrich
    if app.workbench_state.active_context_tab == AlertContextTab::Iocs {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                let len = app
                    .workbench_bundle
                    .as_ref()
                    .map(|b| b.indicators.len())
                    .unwrap_or(0);
                app.workbench_state.move_ioc_selection_down(len);
                return;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.workbench_state.move_ioc_selection_up();
                return;
            }
            KeyCode::Char('y') => {
                copy_selected_ioc(app);
                return;
            }
            KeyCode::Char('e') => {
                enrich_selected_ioc(app);
                return;
            }
            _ => {}
        }
    }

    // Non-IOC tabs: j/k scrolls content
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            app.workbench_state.context_scroll =
                app.workbench_state.context_scroll.saturating_add(1);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.workbench_state.context_scroll =
                app.workbench_state.context_scroll.saturating_sub(1);
        }
        _ => {}
    }
}

// ── IOC helpers ────────────────────────────────────────────────────────────

/// Copy the selected IOC value to the system clipboard.
/// Falls back to showing the value in a notification if no clipboard tool exists.
fn copy_selected_ioc(app: &mut App) {
    let Some(bundle) = app.workbench_bundle.as_ref() else {
        app.set_notification("No alert loaded".into(), NotificationType::Warning);
        return;
    };
    let Some(ind) = bundle
        .indicators
        .get(app.workbench_state.selected_ioc_index)
    else {
        app.set_notification("No IOC selected".into(), NotificationType::Warning);
        return;
    };
    let value = ind.normalized_value.clone();
    match try_copy_to_clipboard(&value) {
        Ok(()) => {
            app.set_notification(format!("Copied: {}", value), NotificationType::Success);
        }
        Err(_) => {
            // Clipboard tool unavailable — show value so user can copy manually
            app.set_notification(
                format!("Clipboard unavailable. IOC value: {}", value),
                NotificationType::Info,
            );
        }
    }
}

/// Queue enrichment for the selected IOC.
fn enrich_selected_ioc(app: &mut App) {
    let Some(bundle) = app.workbench_bundle.as_ref() else {
        app.set_notification("No alert loaded".into(), NotificationType::Warning);
        return;
    };
    let Some(ind) = bundle
        .indicators
        .get(app.workbench_state.selected_ioc_index)
    else {
        app.set_notification("No IOC selected".into(), NotificationType::Warning);
        return;
    };
    let id = ind.id;
    let label = ind.normalized_value.clone();
    app.enrich_indicator(id, &label);
}

/// Try to copy text to the system clipboard via platform commands.
///
/// Tries `pbcopy` (macOS), `clip.exe` (Windows), `wl-copy` (Wayland),
/// or `xclip` (X11). Returns `Err` if no clipboard tool is available.
fn try_copy_to_clipboard(text: &str) -> Result<(), ()> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let (program, args): (&str, &[&str]) = if cfg!(target_os = "macos") {
        ("pbcopy", &[])
    } else if cfg!(target_os = "windows") {
        ("clip.exe", &[])
    } else if std::env::var("WAYLAND_DISPLAY").is_ok() {
        ("wl-copy", &[])
    } else {
        ("xclip", &["-selection", "clipboard"])
    };

    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|_| ())?;

    child
        .stdin
        .take()
        .ok_or(())?
        .write_all(text.as_bytes())
        .map_err(|_| ())?;

    let _ = child.wait();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, Paths};
    use crate::db::{AlertCreate, Db, FeedCreate, KeywordCreate};
    use crate::types::FeedType;
    use crate::ui::alerts::state::{AlertContextTab, AlertPane};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::path::PathBuf;

    fn temp_db_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "threatdeck-events-{}-{}.db",
            name,
            std::process::id()
        ))
    }

    fn make_key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn setup_app(count: usize, path: &std::path::Path) -> App {
        let db = Db::open(path).unwrap();
        db.init_schema().unwrap();
        let feed_id = db
            .create_feed(&FeedCreate {
                name: "TestFeed".into(),
                url: "https://example.com/feed.xml".into(),
                feed_type: FeedType::Rss,
                enabled: true,
                interval_secs: 300,
                ..FeedCreate::default()
            })
            .unwrap();
        let keyword_id = db
            .create_keyword(&KeywordCreate {
                pattern: "breach".into(),
                criticality: Criticality::High,
                enabled: true,
                ..KeywordCreate::default()
            })
            .unwrap();
        for i in 0..count {
            db.create_alert(&AlertCreate {
                feed_id,
                keyword_id,
                title: Some(format!("Alert #{}", i)),
                content_snippet: format!("Activity #{}", i),
                criticality: Criticality::High,
                content_hash: format!("hash-{}", i),
                metadata_json: None,
            })
            .unwrap();
        }
        let paths = Paths {
            config_dir: PathBuf::new(),
            data_dir: PathBuf::new(),
            config_file: PathBuf::new(),
            db_file: path.to_path_buf(),
        };
        let mut app = App::new(db, AppConfig::default(), paths);
        app.screen = crate::types::Screen::Alerts;
        app
    }

    #[test]
    fn tab_cycles_pane_focus() {
        let path = temp_db_path("tab");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app(3, &path);
        assert_eq!(app.workbench_state.focused_pane, AlertPane::List);
        handle_workbench_key(&mut app, make_key(KeyCode::Tab));
        assert_eq!(app.workbench_state.focused_pane, AlertPane::Details);
        handle_workbench_key(&mut app, make_key(KeyCode::Tab));
        assert_eq!(app.workbench_state.focused_pane, AlertPane::Context);
        handle_workbench_key(&mut app, make_key(KeyCode::Tab));
        assert_eq!(app.workbench_state.focused_pane, AlertPane::List);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn shift_tab_cycles_pane_focus_backward() {
        let path = temp_db_path("sftab");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app(3, &path);
        assert_eq!(app.workbench_state.focused_pane, AlertPane::List);
        handle_workbench_key(&mut app, make_key(KeyCode::BackTab));
        assert_eq!(app.workbench_state.focused_pane, AlertPane::Context);
        handle_workbench_key(&mut app, make_key(KeyCode::BackTab));
        assert_eq!(app.workbench_state.focused_pane, AlertPane::Details);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn j_moves_selection_down() {
        let path = temp_db_path("j");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app(3, &path);
        assert_eq!(app.workbench_state.selected_alert_index, 0);
        handle_workbench_key(&mut app, make_key(KeyCode::Char('j')));
        assert_eq!(app.workbench_state.selected_alert_index, 1);
        // Bundle should have reloaded for the new selection
        assert!(app.workbench_bundle.is_some());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn k_moves_selection_up() {
        let path = temp_db_path("k");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app(3, &path);
        app.workbench_state.selected_alert_index = 2;
        app.alerts_selected = 2;
        handle_workbench_key(&mut app, make_key(KeyCode::Char('k')));
        assert_eq!(app.workbench_state.selected_alert_index, 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn selection_clamps_at_bounds() {
        let path = temp_db_path("clamp");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app(1, &path);
        let len = app.workbench_state.alerts.len();
        assert!(len > 0, "expected at least one alert");
        // Move to bottom — j should not move past last alert
        app.workbench_state.selected_alert_index = len - 1;
        app.alerts_selected = len - 1;
        handle_workbench_key(&mut app, make_key(KeyCode::Char('j')));
        assert_eq!(app.workbench_state.selected_alert_index, len - 1);
        // Move to top — k should not move before first alert
        app.workbench_state.selected_alert_index = 0;
        app.alerts_selected = 0;
        handle_workbench_key(&mut app, make_key(KeyCode::Char('k')));
        assert_eq!(app.workbench_state.selected_alert_index, 0);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn no_alerts_j_k_no_panic() {
        let path = temp_db_path("empty");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app(0, &path);
        handle_workbench_key(&mut app, make_key(KeyCode::Char('j')));
        handle_workbench_key(&mut app, make_key(KeyCode::Char('k')));
        assert_eq!(app.workbench_state.selected_alert_index, 0);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn bracket_cycles_context_tabs() {
        let path = temp_db_path("brackets");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app(1, &path);
        app.workbench_state.focused_pane = AlertPane::Context;
        assert_eq!(
            app.workbench_state.active_context_tab,
            AlertContextTab::Iocs
        );
        handle_workbench_key(&mut app, make_key(KeyCode::Char(']')));
        assert_eq!(
            app.workbench_state.active_context_tab,
            AlertContextTab::Metadata
        );
        handle_workbench_key(&mut app, make_key(KeyCode::Char(']')));
        assert_eq!(
            app.workbench_state.active_context_tab,
            AlertContextTab::TriageHistory
        );
        handle_workbench_key(&mut app, make_key(KeyCode::Char('[')));
        assert_eq!(
            app.workbench_state.active_context_tab,
            AlertContextTab::Metadata
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn details_pane_jk_scrolls() {
        let path = temp_db_path("scroll");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app(1, &path);
        app.workbench_state.focused_pane = AlertPane::Details;
        app.workbench_state.detail_scroll = 0;
        handle_workbench_key(&mut app, make_key(KeyCode::Char('j')));
        assert_eq!(app.workbench_state.detail_scroll, 1);
        handle_workbench_key(&mut app, make_key(KeyCode::Char('j')));
        assert_eq!(app.workbench_state.detail_scroll, 2);
        handle_workbench_key(&mut app, make_key(KeyCode::Char('k')));
        assert_eq!(app.workbench_state.detail_scroll, 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn context_pane_jk_scrolls() {
        // On non-IOC tabs, j/k scrolls content
        let path = temp_db_path("cscroll");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app(1, &path);
        app.workbench_state.focused_pane = AlertPane::Context;
        app.workbench_state.active_context_tab = AlertContextTab::Metadata;
        app.workbench_state.context_scroll = 0;
        handle_workbench_key(&mut app, make_key(KeyCode::Char('j')));
        assert_eq!(app.workbench_state.context_scroll, 1);
        handle_workbench_key(&mut app, make_key(KeyCode::Char('k')));
        assert_eq!(app.workbench_state.context_scroll, 0);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn enter_from_list_focuses_details() {
        let path = temp_db_path("enter");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app(2, &path);
        assert_eq!(app.workbench_state.focused_pane, AlertPane::List);
        handle_workbench_key(&mut app, make_key(KeyCode::Enter));
        assert_eq!(app.workbench_state.focused_pane, AlertPane::Details);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn r_refreshes_data() {
        let path = temp_db_path("refresh");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app(2, &path);
        let original_count = app.workbench_state.alerts.len();
        // Add a new alert directly to the DB
        let feed_id = app.alerts_list[0].alert.feed_id;
        let keyword_id = app.alerts_list[0].alert.keyword_id;
        app.db
            .create_alert(&AlertCreate {
                feed_id,
                keyword_id,
                title: Some("New Alert".into()),
                content_snippet: "new".into(),
                criticality: Criticality::High,
                content_hash: "new-hash".into(),
                metadata_json: None,
            })
            .unwrap();
        // Press r to refresh
        handle_workbench_key(&mut app, make_key(KeyCode::Char('r')));
        assert_eq!(app.workbench_state.alerts.len(), original_count + 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn selection_preserved_across_refresh() {
        let path = temp_db_path("preserve");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app(3, &path);
        // Move to second alert
        handle_workbench_key(&mut app, make_key(KeyCode::Char('j')));
        assert_eq!(app.workbench_state.selected_alert_index, 1);
        let selected_id = app.workbench_state.selected_alert_id();
        // Refresh
        handle_workbench_key(&mut app, make_key(KeyCode::Char('r')));
        // Same alert should still be selected
        assert_eq!(app.workbench_state.selected_alert_id(), selected_id);
        let _ = std::fs::remove_file(path);
    }

    // ── Phase 4: Analyst Actions tests ───────────────────────────────────────

    /// Helper: add indicators to the first alert so IOC tests have data.
    fn add_iocs_to_first_alert(app: &mut App, count: usize) {
        let alert_id = app.workbench_state.selected_alert_id().unwrap();
        let indicators: Vec<sentinel_ioc::ExtractedIndicator> = (0..count)
            .map(|i| sentinel_ioc::ExtractedIndicator {
                indicator_type: sentinel_ioc::IndicatorType::Cve,
                value: format!("CVE-2025-{}", 10000 + i),
                normalized_value: format!("CVE-2025-{}", 10000 + i),
                source_field: "content_snippet".into(),
                start_offset: 0,
                end_offset: 13,
                surrounding_text: String::new(),
                confidence_hint: Some(90),
            })
            .collect();
        app.db
            .store_extracted_indicators(&indicators, Some(alert_id), None, None)
            .unwrap();
        // Reload bundle so indicators appear
        app.refresh_workbench_bundle();
    }

    #[test]
    fn context_pane_ioc_tab_jk_moves_ioc_selection() {
        let path = temp_db_path("iocsel");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app(1, &path);
        add_iocs_to_first_alert(&mut app, 3);
        app.workbench_state.focused_pane = AlertPane::Context;
        app.workbench_state.active_context_tab = AlertContextTab::Iocs;
        assert_eq!(app.workbench_state.selected_ioc_index, 0);
        handle_workbench_key(&mut app, make_key(KeyCode::Char('j')));
        assert_eq!(app.workbench_state.selected_ioc_index, 1);
        handle_workbench_key(&mut app, make_key(KeyCode::Char('j')));
        assert_eq!(app.workbench_state.selected_ioc_index, 2);
        // Clamp at bottom
        handle_workbench_key(&mut app, make_key(KeyCode::Char('j')));
        assert_eq!(app.workbench_state.selected_ioc_index, 2);
        // Move up
        handle_workbench_key(&mut app, make_key(KeyCode::Char('k')));
        assert_eq!(app.workbench_state.selected_ioc_index, 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn x_exports_selected_alert() {
        let path = temp_db_path("export");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app(1, &path);
        handle_workbench_key(&mut app, make_key(KeyCode::Char('x')));
        // Export should succeed — notification set to Success
        let (msg, ntype) = app.notification.as_ref().unwrap();
        assert_eq!(*ntype, NotificationType::Success);
        assert!(msg.contains("Exported"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn export_no_alert_shows_warning() {
        let path = temp_db_path("noalert_export");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app(0, &path);
        app.workbench_state.selected_alert_index = 999;
        app.alerts_selected = 999;
        handle_workbench_key(&mut app, make_key(KeyCode::Char('x')));
        let (msg, ntype) = app.notification.as_ref().unwrap();
        assert_eq!(*ntype, NotificationType::Warning);
        assert!(msg.contains("No alert selected"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn y_copies_ioc_value() {
        let path = temp_db_path("copy");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app(1, &path);
        add_iocs_to_first_alert(&mut app, 2);
        app.workbench_state.focused_pane = AlertPane::Context;
        app.workbench_state.active_context_tab = AlertContextTab::Iocs;
        handle_workbench_key(&mut app, make_key(KeyCode::Char('y')));
        // Clipboard tool may or may not exist — either "Copied:" or "Clipboard unavailable"
        // In both cases the IOC value should appear in the notification
        let (msg, _) = app.notification.as_ref().unwrap();
        assert!(msg.contains("CVE-2025-10000"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn y_with_no_ioc_shows_warning() {
        let path = temp_db_path("noioc");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app(1, &path);
        app.workbench_state.focused_pane = AlertPane::Context;
        app.workbench_state.active_context_tab = AlertContextTab::Iocs;
        handle_workbench_key(&mut app, make_key(KeyCode::Char('y')));
        // No IOC loaded in bundle — should show warning or info
        let (_, ntype) = app.notification.as_ref().unwrap();
        assert!(matches!(
            *ntype,
            NotificationType::Warning | NotificationType::Info
        ));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn e_enriches_selected_ioc() {
        let path = temp_db_path("enrich");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app(1, &path);
        add_iocs_to_first_alert(&mut app, 1);
        app.workbench_state.focused_pane = AlertPane::Context;
        app.workbench_state.active_context_tab = AlertContextTab::Iocs;
        handle_workbench_key(&mut app, make_key(KeyCode::Char('e')));
        // Enrichment config may be enabled (Success/Info) or disabled (Warning)
        let (_, ntype) = app.notification.as_ref().unwrap();
        assert!(matches!(
            *ntype,
            NotificationType::Success | NotificationType::Info | NotificationType::Warning
        ));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn ioc_selection_resets_on_alert_change() {
        let path = temp_db_path("iocreset");
        let _ = std::fs::remove_file(&path);
        let mut app = setup_app(3, &path);
        // Move IOC selection to a non-zero index
        app.workbench_state.selected_ioc_index = 2;
        // Change alert selection in the list pane
        handle_workbench_key(&mut app, make_key(KeyCode::Char('j')));
        // IOC selection should have reset to 0
        assert_eq!(app.workbench_state.selected_ioc_index, 0);
        let _ = std::fs::remove_file(path);
    }
}

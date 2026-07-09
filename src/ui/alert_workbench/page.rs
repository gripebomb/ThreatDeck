//! Split-pane alert workbench page — composes the three panes (Ticket 04/05/06)
//! into the live `Screen::Alerts` using the Phase 1 layout engine and app
//! services.
//!
//! This is the Phase 2 assembly: `draw` lays the list / details / context panes
//! out via [`compute_layout`] (wide split, or a single focused pane on
//! narrow/tiny terminals) and renders a key-hint status bar; `handle_key`
//! wires the minimal navigation the foundation already supports (selection,
//! pane focus, context tabs, scroll, refresh). Full key polish is Ticket 07
//! and triage/export actions are Ticket 09.
//!
//! See `docs/MASTER_PLAN.md` (Phase 2) and `docs/ARCHITECTURE.md`.

use crate::types::TriageEnumTarget;
use crate::types::TriageInputTarget;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::App;
use crate::ui::alert_workbench::alert_details::draw_alert_details;
use crate::ui::alert_workbench::alert_list::draw_alert_list;
use crate::ui::alert_workbench::context_tabs::draw_context_panel;
use crate::ui::alert_workbench::state::AlertPane;
use crate::ui::alert_workbench::triage;
use crate::ui::alert_workbench::{compute_layout, LayoutMode};
use crate::ui::list::motion_from_key;

/// Render the assembled split-pane alert workbench.
///
/// Reserves the top row for the global nav tab bar (drawn over by `ui::draw`)
/// and a bottom status row, then splits the remaining content area via
/// [`compute_layout`]. Wide terminals show all three panes; narrow/tiny
/// terminals show only the focused pane (cycle with `Tab`).
pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    if area.height < 3 {
        return;
    }

    let content = Rect {
        x: 0,
        y: 1,
        width: area.width,
        height: area.height.saturating_sub(2),
    };
    let status_area = Rect {
        x: 0,
        y: area.height.saturating_sub(1),
        width: area.width,
        height: 1,
    };

    let focused = app.workbench.focused_pane;
    let layout = compute_layout(content, focused);

    match layout.mode {
        LayoutMode::Wide => {
            draw_alert_list(
                f,
                layout.alert_list,
                &app.workbench_items,
                &app.workbench,
                app.theme,
            );
            draw_alert_details(
                f,
                layout.alert_details,
                app.workbench_bundle
                    .as_ref()
                    .and_then(|b| b.detail.as_ref()),
                &app.workbench,
                app.theme,
            );
            draw_context_panel(
                f,
                layout.context_panel,
                app.workbench_bundle.as_ref(),
                &app.workbench,
                app.theme,
            );
        }
        // Narrow / tiny: show the focused pane across the full content area.
        LayoutMode::Narrow | LayoutMode::Tiny => match focused {
            AlertPane::AlertList => {
                draw_alert_list(f, content, &app.workbench_items, &app.workbench, app.theme)
            }
            AlertPane::AlertDetails => draw_alert_details(
                f,
                content,
                app.workbench_bundle
                    .as_ref()
                    .and_then(|b| b.detail.as_ref()),
                &app.workbench,
                app.theme,
            ),
            AlertPane::ContextPanel => draw_context_panel(
                f,
                content,
                app.workbench_bundle.as_ref(),
                &app.workbench,
                app.theme,
            ),
        },
    }

    draw_status_bar(f, status_area, app);

    // Triage input popups (enum selector / note entry) overlay the workbench.
    triage::draw_overlays(f, app);
}

/// One-line status + key-hint bar. Shows a load error (red) when present,
/// otherwise an alert/unread count for orientation, followed by key hints.
fn draw_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let focus = match app.workbench.focused_pane {
        AlertPane::AlertList => "List",
        AlertPane::AlertDetails => "Details",
        AlertPane::ContextPanel => "Context",
    };
    let tab = app.workbench.bottom_tab.label();

    let (status_text, status_color) = if let Some(err) = &app.workbench.last_error {
        (
            truncate_for_status(&format!("⚠ {err}"), area.width),
            app.theme.error,
        )
    } else {
        let n = app.workbench_items.len();
        let unread = app.workbench_unread_count;
        let label = if unread > 0 {
            format!(" {n} alerts · {unread} unread ")
        } else {
            format!(" {n} alerts ")
        };
        (label, app.theme.muted)
    };

    let full_hint = format!(
        " [Focus: {focus} | Tab: {tab}]   A/I/E/C/O triage · T disp · N note · M owner · x export · j/k · Tab · [/] · r · ? help "
    );
    // Truncate the hint so status + hint never overflow the bar on narrow
    // terminals (char-based; the hint is all single-cell glyphs).
    let hint_budget = (area.width as usize).saturating_sub(status_text.chars().count());
    let hint = truncate_to_width(&full_hint, hint_budget);

    let bar = Paragraph::new(Line::from(vec![
        Span::styled(status_text, Style::default().fg(status_color)),
        Span::styled(hint, Style::default().fg(app.theme.muted)),
    ]))
    .style(
        Style::default()
            .bg(app.theme.surface)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(bar, area);
}

/// Truncate `msg` to at most `budget` display cells, appending an ellipsis when
/// it is shortened. Char-based (safe for the single-cell glyphs used here).
fn truncate_to_width(msg: &str, budget: usize) -> String {
    if msg.chars().count() <= budget {
        return msg.to_string();
    }
    if budget == 0 {
        return String::new();
    }
    let truncated: String = msg.chars().take(budget.saturating_sub(1)).collect();
    format!("{truncated}…")
}

/// Truncate a status-bar error message so it leaves room for the key hints
/// (~40% of the bar width).
fn truncate_for_status(msg: &str, width: u16) -> String {
    let budget = (width as usize).saturating_mul(4) / 10;
    truncate_to_width(msg, budget)
}

/// Half-page scroll increment (matches the shared list motion helper).
const SCROLL_HALF: i32 = 10;

/// Workbench keyboard navigation (Ticket 07). Global keys (screen switching,
/// quit, help, filter, command palette, Esc) are handled earlier in
/// `App::handle_key` and never reach here.
///
/// Model: `j/k/arrows/gg/G/Ctrl-d/u` move the alert-list selection (the primary
/// navigation) regardless of focus; `J/K/Ctrl-d/u/PgUp/Dn` scroll the focused
/// details/context pane; `Tab/Shift+Tab` cycle pane focus; `[/]` cycle context
/// tabs; `r` refreshes.
///
/// Note: the ticket's `1/2/3` direct-pane-focus mapping is intentionally NOT
/// bound — `1`/`2`/`3` are the app's global screen-switch hotkeys. Focus is
/// handled by `Tab`/`Shift+Tab`.
pub fn handle_key(app: &mut App, key: KeyEvent) {
    // 0) Triage input modes (enum selector / note entry) take over all keys.
    if triage::handle_input_modes(app, key) {
        return;
    }

    // 1) Pane-local scrolling when a scrollable pane (details/context) is focused.
    if app.workbench.focused_pane != AlertPane::AlertList {
        if let Some(delta) = scroll_delta(key) {
            scroll_focused(app, delta);
            return;
        }
    }

    // 2) Alert-list selection via the shared motion helper (j/k/arrows/gg/G/
    //    Ctrl-d/u). Also reloads the selected alert's bundle.
    if let Some(motion) = motion_from_key(key, &mut app.pending_g) {
        app.workbench_move_selection(motion);
        return;
    }

    // 3) Pane focus, context tabs, refresh, and triage/export actions (T09).
    match key.code {
        KeyCode::Tab => app.workbench.focus_next_pane(),
        KeyCode::BackTab => app.workbench.focus_prev_pane(),
        KeyCode::Char(']') => app.workbench.cycle_tab_forward(),
        KeyCode::Char('[') => app.workbench.cycle_tab_backward(),
        KeyCode::Char('r') => app.refresh_workbench(),
        // Triage: status
        KeyCode::Char('A') => triage::acknowledge(app),
        KeyCode::Char('I') => triage::investigate(app),
        KeyCode::Char('E') => triage::escalate(app),
        KeyCode::Char('C') => triage::close(app),
        KeyCode::Char('O') => triage::reopen(app),
        // Triage: enum selectors
        KeyCode::Char('s') => triage::start_enum_select(app, TriageEnumTarget::Status),
        KeyCode::Char('T') => triage::start_enum_select(app, TriageEnumTarget::Disposition),
        KeyCode::Char('S') => triage::start_enum_select(app, TriageEnumTarget::Severity),
        // Triage: free-text entry
        KeyCode::Char('N') => triage::start_note_input(app, TriageInputTarget::Note),
        KeyCode::Char('M') => triage::start_note_input(app, TriageInputTarget::Owner),
        // Export selected alert to Markdown
        KeyCode::Char('x') => triage::export_selected(app),
        _ => {}
    }
}

/// Translate a scroll key (only meaningful for details/context panes) to a
/// line delta, if any.
fn scroll_delta(key: KeyEvent) -> Option<i32> {
    match key.code {
        KeyCode::Char('J') => Some(1),
        KeyCode::Char('K') => Some(-1),
        KeyCode::PageDown => Some(SCROLL_HALF),
        KeyCode::PageUp => Some(-SCROLL_HALF),
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(SCROLL_HALF),
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(-SCROLL_HALF),
        _ => None,
    }
}

/// Scroll the focused details/context pane. The stored offset is unbounded
/// here; each renderer clamps the *displayed* offset to its content height.
fn scroll_focused(app: &mut App, delta: i32) {
    const UNBOUNDED: u16 = u16::MAX;
    match app.workbench.focused_pane {
        AlertPane::AlertDetails => app.workbench.scroll_detail(delta, UNBOUNDED, 1),
        AlertPane::ContextPanel => app.workbench.scroll_context(delta, UNBOUNDED, 1),
        AlertPane::AlertList => {}
    }
}

#[cfg(test)]
mod tests {
    //! Smoke tests for the assembled workbench page.
    use super::*;
    use crate::app::InputMode;
    use crate::config::{AppConfig, Paths};
    use crate::db::{AlertCreate, Db, FeedCreate, KeywordCreate};
    use crate::types::{
        AlertDisposition, AlertStatus, Criticality, FeedType, NotificationType, Screen,
    };
    use crossterm::event::KeyModifiers;
    use ratatui::{backend::TestBackend, Terminal};
    use std::path::PathBuf;

    fn temp_db(name: &str) -> (Db, PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "threatdeck-wb-page-{}-{}.db",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let db = Db::open(&path).unwrap();
        db.init_schema().unwrap();
        (db, path)
    }

    fn seed(db: &Db, n: usize) {
        let feed_id = db
            .create_feed(&FeedCreate {
                name: "IOCFeed".into(),
                url: "https://ioc.example.test/feed.xml".into(),
                feed_type: FeedType::Rss,
                enabled: true,
                interval_secs: 300,
                ..FeedCreate::default()
            })
            .unwrap();
        let keyword_id = db
            .create_keyword(&KeywordCreate {
                pattern: "ransomware".into(),
                criticality: Criticality::High,
                enabled: true,
                ..KeywordCreate::default()
            })
            .unwrap();
        for i in 0..n {
            db.create_alert(&AlertCreate {
                feed_id,
                keyword_id,
                title: Some(format!("Alert #{i}")),
                content_snippet: format!("Ransomware mentions CVE-2025-100{i}"),
                criticality: Criticality::High,
                content_hash: format!("hash-{i}"),
                metadata_json: Some(format!(r#"{{"i":{i}}}"#)),
            })
            .unwrap();
        }
    }

    fn build_app(name: &str, n: usize) -> (App, PathBuf) {
        let (db, path) = temp_db(name);
        seed(&db, n);
        let paths = Paths {
            config_dir: PathBuf::new(),
            data_dir: PathBuf::new(),
            config_file: PathBuf::new(),
            db_file: path.clone(),
        };
        let mut app = App::new(db, AppConfig::default(), paths);
        app.screen = Screen::Alerts;
        // init_schema seeds a demo catalog; filter to only the alerts we seeded
        // so list lengths are deterministic.
        app.alerts_filter = "Alert #".to_string();
        app.refresh_workbench();
        (app, path)
    }

    fn render_text(app: &mut App, w: u16, h: u16) -> String {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, app)).unwrap();
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

    #[test]
    fn status_bar_shows_error_when_present() {
        let (mut app, path) = build_app("sberr", 2);
        app.workbench.set_error(Some("database locked".into()));
        let text = render_text(&mut app, 120, 30);
        assert!(text.contains('⚠'), "error glyph missing:\n{text}");
        assert!(
            text.contains("database locked"),
            "error text missing:\n{text}"
        );
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn status_bar_shows_alert_count_when_no_error() {
        let (mut app, path) = build_app("sbcount", 5);
        let text = render_text(&mut app, 120, 30);
        assert!(text.contains("5 alerts"), "count missing:\n{text}");
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn wide_renders_all_three_panes() {
        let (mut app, path) = build_app("wide", 5);
        let text = render_text(&mut app, 120, 34);
        assert!(
            text.contains("Alerts ("),
            "list pane title missing:\n{text}"
        );
        assert!(
            text.contains("Alert Details"),
            "details pane missing:\n{text}"
        );
        assert!(text.contains("Context:"), "context pane missing:\n{text}");
        // Status bar hints render.
        assert!(text.contains("A/I/E/C/O"), "status bar missing:\n{text}");
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn triage_acknowledge_key_routes_through_handler() {
        let (mut app, path) = build_app("triagekey", 2);
        let id = app.workbench.selected_alert_id();
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('A'), KeyModifiers::NONE),
        );
        assert_eq!(
            app.workbench_bundle
                .as_ref()
                .and_then(|b| b.detail.as_ref())
                .unwrap()
                .status,
            AlertStatus::Acknowledged,
            "A key should acknowledge the selected alert"
        );
        assert_eq!(
            app.notification.as_ref().unwrap().1,
            NotificationType::Success
        );
        // Selection survives the refresh.
        assert_eq!(app.workbench.selected_alert_id(), id);
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn triage_disposition_selector_via_keys() {
        let (mut app, path) = build_app("dispkey", 1);
        // `T` opens the disposition selector under Typing mode.
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('T'), KeyModifiers::NONE),
        );
        assert!(app.triage_enum_select_mode, "T should open the selector");
        assert_eq!(app.input_mode, InputMode::Typing);
        // `2` picks ConfirmedThreat.
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE),
        );
        assert!(
            !app.triage_enum_select_mode,
            "selector should close after pick"
        );
        assert_eq!(
            app.workbench_bundle
                .as_ref()
                .and_then(|b| b.detail.as_ref())
                .unwrap()
                .disposition,
            AlertDisposition::ConfirmedThreat
        );
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn narrow_renders_focused_pane_without_panic() {
        let (mut app, path) = build_app("narrow", 3);
        // Focused pane defaults to the list.
        let text = render_text(&mut app, 92, 30);
        assert!(
            text.contains("Alerts ("),
            "focused list pane missing:\n{text}"
        );

        // Cycling focus to details still renders cleanly on a narrow terminal.
        app.workbench.focus_next_pane(); // -> Details
        let text = render_text(&mut app, 92, 30);
        assert!(text.contains("Alert Details"));
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn empty_list_renders_empty_state() {
        let (mut app, path) = build_app("empty", 0);
        // init_schema seeds a demo catalog; filter to nothing to force an empty list.
        app.alerts_filter = "zzzz-no-such-alert".into();
        app.refresh_workbench();
        let text = render_text(&mut app, 120, 34);
        assert!(text.contains("No alerts"), "empty state missing:\n{text}");
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn moving_selection_reloads_bundle() {
        let (mut app, path) = build_app("select", 3);
        let first_id = app.workbench.selected_alert_id();
        assert!(first_id.is_some());
        assert!(app.workbench_bundle.is_some());

        // Move down and confirm the bundle reloads for a different alert.
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
        );
        let next_id = app.workbench.selected_alert_id();
        assert_ne!(first_id, next_id, "selection should have moved");
        assert!(app.workbench_bundle.is_some(), "bundle should reload");
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn tab_focus_and_context_tabs_cycle() {
        let (mut app, path) = build_app("tabs", 2);
        assert_eq!(app.workbench.focused_pane, AlertPane::AlertList);
        handle_key(&mut app, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.workbench.focused_pane, AlertPane::AlertDetails);
        handle_key(&mut app, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.workbench.focused_pane, AlertPane::ContextPanel);

        // Context tabs cycle.
        let first_tab = app.workbench.bottom_tab;
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE),
        );
        assert_ne!(app.workbench.bottom_tab, first_tab);
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn gg_top_and_shift_g_bottom() {
        let (mut app, path) = build_app("gg", 5);
        // Move down a few first.
        for _ in 0..3 {
            handle_key(
                &mut app,
                KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
            );
        }
        assert_eq!(app.workbench.selected_alert_index(), 3);
        // `gg` jumps to top.
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
        );
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
        );
        assert_eq!(app.workbench.selected_alert_index(), 0);
        // `G` jumps to bottom.
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE),
        );
        assert_eq!(app.workbench.selected_alert_index(), 4);
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn ctrl_d_moves_list_selection_half_page() {
        let (mut app, path) = build_app("ctrld", 5);
        assert_eq!(app.workbench.focused_pane, AlertPane::AlertList);
        assert_eq!(app.workbench.selected_alert_index(), 0);
        // Ctrl-d on the list moves the selection (clamped to the last row).
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
        );
        assert_eq!(app.workbench.selected_alert_index(), 4);
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn refresh_preserves_selected_alert() {
        let (mut app, path) = build_app("preserve", 5);
        // Move to a known alert and remember its id.
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
        );
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
        );
        let selected = app.workbench.selected_alert_id();
        let index = app.workbench.selected_alert_index();
        assert!(selected.is_some());

        // Refresh keeps the same alert selected (by id).
        app.refresh_workbench();
        assert_eq!(app.workbench.selected_alert_id(), selected);
        assert_eq!(app.workbench.selected_alert_index(), index);
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn scrolling_keys_scroll_focused_details_pane() {
        let (mut app, path) = build_app("scroll", 2);
        // Focus the details pane.
        handle_key(&mut app, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.workbench.focused_pane, AlertPane::AlertDetails);
        assert_eq!(app.workbench.right_detail_scroll, 0);
        // J scrolls down one line.
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('J'), KeyModifiers::NONE),
        );
        assert_eq!(app.workbench.right_detail_scroll, 1);
        // K scrolls back up.
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('K'), KeyModifiers::NONE),
        );
        assert_eq!(app.workbench.right_detail_scroll, 0);
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn empty_list_does_not_panic_on_keys() {
        let (mut app, path) = build_app("nokeys", 0);
        app.alerts_filter = "zzzz-no-such-alert".into();
        app.refresh_workbench();
        assert!(app.workbench_items.is_empty());
        assert!(app.workbench.selected_alert_id().is_none());

        // None of these should panic; selection stays empty/None.
        for code in [
            KeyCode::Char('j'),
            KeyCode::Char('k'),
            KeyCode::Down,
            KeyCode::Up,
            KeyCode::Tab,
            KeyCode::Char(']'),
            KeyCode::Char('r'),
            KeyCode::Char('G'),
        ] {
            handle_key(&mut app, KeyEvent::new(code, KeyModifiers::NONE));
        }
        assert!(app.workbench.selected_alert_id().is_none());
        assert_eq!(app.workbench.selected_alert_index(), 0);
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn tiny_terminal_does_not_panic() {
        let (mut app, path) = build_app("tiny", 2);
        // Global ui::draw guards <80x24, but the page itself must stay safe.
        let _ = render_text(&mut app, 82, 24);
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn renders_safely_across_terminal_sizes() {
        // Simulates odd / SSH terminal sizes: the assembled page must never
        // panic (defense-in-depth; the global draw guards <80x24 anyway).
        let (mut app, path) = build_app("sizes", 4);
        for &(w, h) in &[
            (1, 1),
            (2, 2),
            (10, 5),
            (20, 8),
            (79, 23),
            (80, 24),
            (109, 29),
            (110, 30),
            (120, 34),
            (200, 60),
        ] {
            let _ = render_text(&mut app, w, h);
        }
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    /// Render via the top-level `ui::draw` (includes the help overlay).
    fn render_full(app: &mut App, w: u16, h: u16) -> String {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| crate::ui::draw(f, app)).unwrap();
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

    #[test]
    fn help_overlay_documents_workbench_keys() {
        let (mut app, path) = build_app("help", 2);
        app.show_help = true;
        let text = render_full(&mut app, 120, 40);
        assert!(
            text.contains("Alerts Workbench"),
            "help overlay missing workbench section:\n{text}"
        );
        assert!(
            text.contains("A I E C O"),
            "triage keys missing from help:\n{text}"
        );
        assert!(
            text.contains("Export Markdown"),
            "export missing from help:\n{text}"
        );
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn focused_pane_title_carries_focus_marker() {
        // Accessibility: focus is conveyed by a glyph, not colour alone.
        let (mut app, path) = build_app("marker", 2);
        // Default focus is the list pane.
        let text = render_text(&mut app, 120, 34);
        assert!(
            text.contains('◆'),
            "focused (list) pane should carry the focus marker:\n{text}"
        );
        drop(app);
        let _ = std::fs::remove_file(path);
    }
}

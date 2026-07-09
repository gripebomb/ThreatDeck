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
use crate::ui::alert_workbench::{compute_layout, LayoutMode};
use crate::ui::list::{motion_from_key, move_selection};

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

    let status: Vec<Span> = if let Some(err) = &app.workbench.last_error {
        let msg = truncate_for_status(&format!("⚠ {err}"), area.width);
        vec![Span::styled(
            format!(" {msg} "),
            Style::default().fg(app.theme.error),
        )]
    } else {
        let n = app.workbench_items.len();
        let unread = app.workbench_items.iter().filter(|i| !i.read).count();
        let label = if unread > 0 {
            format!(" {n} alerts · {unread} unread ")
        } else {
            format!(" {n} alerts ")
        };
        vec![Span::styled(label, Style::default().fg(app.theme.muted))]
    };

    let hint = format!(
        "j/k move · Tab focus · [/] tabs · J/K or Ctrl-d/u scroll · r refresh · / filter · ? help · q quit   [Focus: {focus} | Tab: {tab}] "
    );

    let mut line_spans = status;
    line_spans.push(Span::styled(hint, Style::default().fg(app.theme.muted)));

    let bar = Paragraph::new(Line::from(line_spans)).style(
        Style::default()
            .bg(app.theme.surface)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(bar, area);
}

/// Truncate a status-bar message so it leaves room for the key hints
/// (~40% of the bar width). Char-based to stay multibyte-safe.
fn truncate_for_status(msg: &str, width: u16) -> String {
    let budget = (width as usize).saturating_mul(4) / 10;
    if msg.chars().count() <= budget {
        return msg.to_string();
    }
    let truncated: String = msg.chars().take(budget.saturating_sub(1)).collect();
    format!("{truncated}…")
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
    let len = app.workbench_items.len();

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
        app.workbench.selected_alert_index =
            move_selection(app.workbench.selected_alert_index, len, motion);
        app.workbench_reload_selected();
        return;
    }

    // 3) Pane focus, context tabs, refresh.
    match key.code {
        KeyCode::Tab => app.workbench.focus_next_pane(),
        KeyCode::BackTab => app.workbench.focus_prev_pane(),
        KeyCode::Char(']') => app.workbench.cycle_tab_forward(),
        KeyCode::Char('[') => app.workbench.cycle_tab_backward(),
        KeyCode::Char('r') => app.refresh_workbench(),
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
    use crate::config::{AppConfig, Paths};
    use crate::db::{AlertCreate, Db, FeedCreate, KeywordCreate};
    use crate::types::{Criticality, FeedType, Screen};
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
        assert!(text.contains("j/k move"), "status bar missing:\n{text}");
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
        let first_id = app.workbench.selected_alert_id;
        assert!(first_id.is_some());
        assert!(app.workbench_bundle.is_some());

        // Move down and confirm the bundle reloads for a different alert.
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
        );
        let next_id = app.workbench.selected_alert_id;
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
        assert_eq!(app.workbench.selected_alert_index, 3);
        // `gg` jumps to top.
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
        );
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
        );
        assert_eq!(app.workbench.selected_alert_index, 0);
        // `G` jumps to bottom.
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE),
        );
        assert_eq!(app.workbench.selected_alert_index, 4);
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn ctrl_d_moves_list_selection_half_page() {
        let (mut app, path) = build_app("ctrld", 5);
        assert_eq!(app.workbench.focused_pane, AlertPane::AlertList);
        assert_eq!(app.workbench.selected_alert_index, 0);
        // Ctrl-d on the list moves the selection (clamped to the last row).
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
        );
        assert_eq!(app.workbench.selected_alert_index, 4);
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
        let selected = app.workbench.selected_alert_id;
        let index = app.workbench.selected_alert_index;
        assert!(selected.is_some());

        // Refresh keeps the same alert selected (by id).
        app.refresh_workbench();
        assert_eq!(app.workbench.selected_alert_id, selected);
        assert_eq!(app.workbench.selected_alert_index, index);
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
        assert!(app.workbench.selected_alert_id.is_none());

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
        assert!(app.workbench.selected_alert_id.is_none());
        assert_eq!(app.workbench.selected_alert_index, 0);
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
}

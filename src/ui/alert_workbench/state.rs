//! Workbench state for the split-pane alert view.
//!
//! Owns pane focus, context-tab selection, alert selection, scroll offsets,
//! filters, sort mode, and loading/error flags. Pure state — no rendering, no
//! SQL. See `tickets/01-alert-workbench-state.md`.
//!
//! Default focused pane: [`AlertPane::AlertList`].
//! Default context tab: [`AlertContextTab::Indicators`] (the "IOCs" tab).

use crate::types::{AlertDisposition, AlertStatus, Criticality};

// ── Pane focus ───────────────────────────────────────────────────────────────

/// The three interactive panes of the workbench. Cycling order is list →
/// details → context → list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlertPane {
    #[default]
    AlertList,
    AlertDetails,
    ContextPanel,
}

impl AlertPane {
    /// All panes in cycle order.
    pub const ALL: [AlertPane; 3] = [
        AlertPane::AlertList,
        AlertPane::AlertDetails,
        AlertPane::ContextPanel,
    ];

    pub fn next(self) -> AlertPane {
        let idx = Self::index_of(self);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> AlertPane {
        let idx = Self::index_of(self);
        let len = Self::ALL.len();
        Self::ALL[(idx + len - 1) % len]
    }

    fn index_of(self) -> usize {
        Self::ALL
            .iter()
            .position(|&p| p == self)
            .expect("AlertPane::ALL contains every variant")
    }

    /// Human-readable title for the focused border.
    pub fn title(self) -> &'static str {
        match self {
            AlertPane::AlertList => "Alerts",
            AlertPane::AlertDetails => "Alert Details",
            AlertPane::ContextPanel => "Context",
        }
    }
}

// ── Context tabs ─────────────────────────────────────────────────────────────

/// Bottom-right context panel tabs. Cycling order matches [`Self::ALL`].
///
/// For MVP only Indicators (IOCs), Metadata, and TriageHistory render with
/// data; Enrichment and RawContent are future-ready placeholders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlertContextTab {
    /// The IOCs tab (linked indicators).
    #[default]
    Indicators,
    Metadata,
    Enrichment,
    TriageHistory,
    RawContent,
}

impl AlertContextTab {
    pub const ALL: [AlertContextTab; 5] = [
        AlertContextTab::Indicators,
        AlertContextTab::Metadata,
        AlertContextTab::Enrichment,
        AlertContextTab::TriageHistory,
        AlertContextTab::RawContent,
    ];

    pub fn next(self) -> AlertContextTab {
        let idx = Self::index_of(self);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> AlertContextTab {
        let idx = Self::index_of(self);
        let len = Self::ALL.len();
        Self::ALL[(idx + len - 1) % len]
    }

    fn index_of(self) -> usize {
        Self::ALL
            .iter()
            .position(|&t| t == self)
            .expect("AlertContextTab::ALL contains every variant")
    }

    pub fn label(self) -> &'static str {
        match self {
            AlertContextTab::Indicators => "IOCs",
            AlertContextTab::Metadata => "Metadata",
            AlertContextTab::Enrichment => "Enrichment",
            AlertContextTab::TriageHistory => "History",
            AlertContextTab::RawContent => "Raw",
        }
    }
}

// ── Filter / sort ────────────────────────────────────────────────────────────

/// Workbench-owned alert filter knobs. The app service converts these into a
/// storage [`crate::db::AlertFilter`]; the TUI never builds SQL or storage types.
#[derive(Debug, Clone, Default)]
pub struct AlertFilterState {
    pub text: String,
    pub severity: Option<Criticality>,
    pub status: Option<AlertStatus>,
    pub disposition: Option<AlertDisposition>,
    pub unread_only: bool,
    pub hide_closed: bool,
}

impl AlertFilterState {
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
            && self.severity.is_none()
            && self.status.is_none()
            && self.disposition.is_none()
            && !self.unread_only
            && !self.hide_closed
    }
}

/// Alert list sort order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlertSortMode {
    /// Newest first (default; matches existing `detected_at DESC`).
    #[default]
    NewestFirst,
    OldestFirst,
    SeverityDesc,
    SeverityAsc,
}

// ── Scroll helpers ───────────────────────────────────────────────────────────

/// Clamp a scroll offset so the last viewport of content stays visible.
/// Returns 0 when content fits the viewport.
pub fn clamp_scroll(scroll: u16, content_lines: u16, viewport_lines: u16) -> u16 {
    if content_lines <= viewport_lines {
        0
    } else {
        scroll.min(content_lines - viewport_lines)
    }
}

// ── Workbench state ──────────────────────────────────────────────────────────

/// Central state for the split-pane alert workbench. See
/// `tickets/01-alert-workbench-state.md`.
#[derive(Debug, Clone, Default)]
pub struct AlertWorkbenchState {
    /// Stable id of the selected alert, preserved across refreshes where possible.
    ///
    /// Private on purpose: it must stay in sync with `selected_alert_index`.
    /// All changes flow through [`Self::set_selection`] / [`Self::select_at`] /
    /// [`Self::restore_selection_by_id`], which update the pair together so a
    /// desync is unrepresentable. Read via [`Self::selected_alert_id`].
    selected_alert_id: Option<i64>,
    /// Index into the loaded alert list (clamped to list bounds). Private for
    /// the same reason as `selected_alert_id`; read via
    /// [`Self::selected_alert_index`].
    selected_alert_index: usize,
    /// Vertical scroll of the left alert list (in rows).
    pub alert_list_scroll: usize,
    /// Currently focused pane.
    pub focused_pane: AlertPane,
    /// Vertical scroll of the top-right details pane.
    pub right_detail_scroll: u16,
    /// Vertical scroll of the bottom-right context pane.
    pub bottom_detail_scroll: u16,
    /// Active bottom-right context tab.
    pub bottom_tab: AlertContextTab,
    pub alert_filter: AlertFilterState,
    pub alert_sort: AlertSortMode,
    /// Last error loading the selected-alert bundle, if any.
    pub last_error: Option<String>,
}

impl AlertWorkbenchState {
    pub fn new() -> Self {
        Self::default()
    }

    // ── Pane focus ───────────────────────────────────────────────────────────

    pub fn focus_next_pane(&mut self) {
        self.focused_pane = self.focused_pane.next();
    }

    pub fn focus_prev_pane(&mut self) {
        self.focused_pane = self.focused_pane.prev();
    }

    pub fn set_focused_pane(&mut self, pane: AlertPane) {
        self.focused_pane = pane;
    }

    // ── Context tabs ─────────────────────────────────────────────────────────

    pub fn cycle_tab_forward(&mut self) {
        self.bottom_tab = self.bottom_tab.next();
        self.reset_context_scroll();
    }

    pub fn cycle_tab_backward(&mut self) {
        self.bottom_tab = self.bottom_tab.prev();
        self.reset_context_scroll();
    }

    pub fn set_tab(&mut self, tab: AlertContextTab) {
        self.bottom_tab = tab;
        self.reset_context_scroll();
    }

    // ── Alert selection ──────────────────────────────────────────────────────
    //
    // The selection is an (index, id) pair that must stay consistent:
    // `selected_alert_id` is the id of the alert at `selected_alert_index` in
    // the loaded list. The id↔index mapping lives in `App` (which owns the
    // items), so both fields are private and every change flows through a
    // method that updates them together. There are no index-only mutators
    // (the old `move_selection_*` helpers were removed because they could move
    // the index without updating the id).

    /// The selected alert's stable id (preserved across refreshes).
    pub fn selected_alert_id(&self) -> Option<i64> {
        self.selected_alert_id
    }

    /// The index of the selected row in the currently loaded alert list.
    pub fn selected_alert_index(&self) -> usize {
        self.selected_alert_index
    }

    /// Set the selection to `desired_index` (clamped to `[0, len-1]`) and derive
    /// `selected_alert_id` via `id_at_index`. This is the single way to move the
    /// selection, so the index↔id invariant can't be broken by callers. Does
    /// not reset detail/context scroll.
    pub fn set_selection(
        &mut self,
        desired_index: usize,
        list_len: usize,
        id_at_index: impl Fn(usize) -> Option<i64>,
    ) {
        // An empty list has no selectable row: clear the id rather than asking
        // the closure for a bogus index-0 id.
        if list_len == 0 {
            self.selected_alert_index = 0;
            self.selected_alert_id = None;
            return;
        }
        let clamped = desired_index.min(list_len - 1);
        self.selected_alert_index = clamped;
        self.selected_alert_id = id_at_index(clamped);
    }

    /// Move selection to a row and record its alert id, then reset
    /// detail/context scroll (a new alert is being shown).
    pub fn select_at(&mut self, index: usize, list_len: usize, alert_id: Option<i64>) {
        let clamped = if list_len == 0 {
            0
        } else {
            index.min(list_len - 1)
        };
        self.selected_alert_index = clamped;
        self.selected_alert_id = alert_id;
        self.reset_scroll_for_selection_change();
    }

    /// After a list refresh, restore the selection to the previously selected
    /// alert id when it still exists, else fall back to the clamped index (and
    /// its id). Returns whether the preserved id was found.
    pub fn restore_selection_by_id(
        &mut self,
        list_len: usize,
        id_at_index: impl Fn(usize) -> Option<i64>,
    ) -> bool {
        if list_len == 0 {
            self.selected_alert_index = 0;
            self.selected_alert_id = None;
            return false;
        }
        if let Some(wanted) = self.selected_alert_id {
            for i in 0..list_len {
                if id_at_index(i) == Some(wanted) {
                    self.selected_alert_index = i;
                    return true;
                }
            }
        }
        // Fall back to the clamped index and reconcile its id so the pair stays
        // consistent even without a follow-up `set_selection`.
        let clamped = self.selected_alert_index.min(list_len - 1);
        self.selected_alert_index = clamped;
        self.selected_alert_id = id_at_index(clamped);
        false
    }

    // ── Scroll ───────────────────────────────────────────────────────────────

    /// Reset detail/context scroll when the selected alert changes.
    pub fn reset_scroll_for_selection_change(&mut self) {
        self.right_detail_scroll = 0;
        self.bottom_detail_scroll = 0;
    }

    pub fn reset_context_scroll(&mut self) {
        self.bottom_detail_scroll = 0;
    }

    /// Scroll the top-right details pane by `delta` lines, clamped so the last
    /// viewport stays visible. Negative deltas scroll up.
    ///
    /// The arithmetic is `i32`-saturating and clamped to `[0, max_scroll]`
    /// *before* narrowing to `u16`, so a very large positive `delta` can't wrap
    /// through `u16` and land short of the bottom.
    pub fn scroll_detail(&mut self, delta: i32, content_lines: u16, viewport_lines: u16) {
        let max_scroll = content_lines.saturating_sub(viewport_lines) as i32;
        let next = (self.right_detail_scroll as i32).saturating_add(delta);
        self.right_detail_scroll = next.clamp(0, max_scroll) as u16;
    }

    /// Scroll the bottom-right context pane by `delta` lines, clamped (see
    /// [`Self::scroll_detail`] for the overflow-safe clamping).
    pub fn scroll_context(&mut self, delta: i32, content_lines: u16, viewport_lines: u16) {
        let max_scroll = content_lines.saturating_sub(viewport_lines) as i32;
        let next = (self.bottom_detail_scroll as i32).saturating_add(delta);
        self.bottom_detail_scroll = next.clamp(0, max_scroll) as u16;
    }

    // ── Loading / error ──────────────────────────────────────────────────────

    pub fn set_error(&mut self, message: Option<String>) {
        self.last_error = message;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab_seq() -> Vec<AlertContextTab> {
        AlertContextTab::ALL.to_vec()
    }

    // ── Default state ────────────────────────────────────────────────────────

    #[test]
    fn default_state_focuses_alert_list_and_indicators_tab() {
        let s = AlertWorkbenchState::new();
        assert_eq!(s.focused_pane, AlertPane::AlertList);
        assert_eq!(s.bottom_tab, AlertContextTab::Indicators);
        assert!(s.selected_alert_id.is_none());
        assert_eq!(s.selected_alert_index, 0);
        assert!(s.last_error.is_none());
    }

    // ── Pane focus cycling ───────────────────────────────────────────────────

    #[test]
    fn pane_focus_cycles_forward() {
        let mut s = AlertWorkbenchState::new();
        assert_eq!(s.focused_pane, AlertPane::AlertList);
        s.focus_next_pane();
        assert_eq!(s.focused_pane, AlertPane::AlertDetails);
        s.focus_next_pane();
        assert_eq!(s.focused_pane, AlertPane::ContextPanel);
        s.focus_next_pane();
        assert_eq!(s.focused_pane, AlertPane::AlertList);
    }

    #[test]
    fn pane_focus_cycles_backward() {
        let mut s = AlertWorkbenchState::new();
        s.focus_prev_pane();
        assert_eq!(s.focused_pane, AlertPane::ContextPanel);
        s.focus_prev_pane();
        assert_eq!(s.focused_pane, AlertPane::AlertDetails);
        s.focus_prev_pane();
        assert_eq!(s.focused_pane, AlertPane::AlertList);
    }

    #[test]
    fn direct_pane_focus_works() {
        let mut s = AlertWorkbenchState::new();
        s.set_focused_pane(AlertPane::ContextPanel);
        assert_eq!(s.focused_pane, AlertPane::ContextPanel);
    }

    // ── Context tab cycling ──────────────────────────────────────────────────

    #[test]
    fn context_tab_cycles_forward() {
        let mut s = AlertWorkbenchState::new();
        let seq = tab_seq();
        assert_eq!(s.bottom_tab, seq[0]);
        for expected in &seq[1..] {
            s.cycle_tab_forward();
            assert_eq!(s.bottom_tab, *expected);
        }
        // Wrap around to start.
        s.cycle_tab_forward();
        assert_eq!(s.bottom_tab, seq[0]);
    }

    #[test]
    fn context_tab_cycles_backward() {
        let mut s = AlertWorkbenchState::new();
        s.cycle_tab_backward();
        assert_eq!(s.bottom_tab, AlertContextTab::RawContent);
        s.cycle_tab_backward();
        assert_eq!(s.bottom_tab, AlertContextTab::TriageHistory);
        s.cycle_tab_backward();
        assert_eq!(s.bottom_tab, AlertContextTab::Enrichment);
        s.cycle_tab_backward();
        assert_eq!(s.bottom_tab, AlertContextTab::Metadata);
        s.cycle_tab_backward();
        assert_eq!(s.bottom_tab, AlertContextTab::Indicators);
    }

    #[test]
    fn set_tab_resets_context_scroll() {
        let mut s = AlertWorkbenchState::new();
        s.bottom_detail_scroll = 42;
        s.set_tab(AlertContextTab::Metadata);
        assert_eq!(s.bottom_tab, AlertContextTab::Metadata);
        assert_eq!(s.bottom_detail_scroll, 0);
    }

    // ── Alert selection ─────────────────────────────────────────────────────
    //
    // The index↔id invariant is enforced by routing all selection changes
    // through `set_selection` / `select_at` / `restore_selection_by_id`. These
    // cover clamping and id reconciliation.

    fn ids(start: i64) -> impl Fn(usize) -> Option<i64> {
        move |i| Some(start + i as i64)
    }

    #[test]
    fn set_selection_clamps_index_and_derives_id() {
        let mut s = AlertWorkbenchState::new();
        // Out-of-range index clamps to the last row, id derived from it.
        s.set_selection(100, 5, ids(10));
        assert_eq!(s.selected_alert_index(), 4);
        assert_eq!(s.selected_alert_id(), Some(14));
        // In-range index maps to its own id.
        s.set_selection(2, 5, ids(10));
        assert_eq!(s.selected_alert_index(), 2);
        assert_eq!(s.selected_alert_id(), Some(12));
        // Empty list clamps to 0 with no id.
        s.set_selection(3, 0, ids(10));
        assert_eq!(s.selected_alert_index(), 0);
        assert_eq!(s.selected_alert_id(), None);
    }

    #[test]
    fn set_selection_keeps_index_and_id_in_sync() {
        let mut s = AlertWorkbenchState::new();
        // There is no way to set the index without also deriving the id, so the
        // pair can't drift. Every set_selection yields a consistent id-at-index.
        for i in 0..5 {
            s.set_selection(i, 5, ids(100));
            assert_eq!(s.selected_alert_id(), Some(100 + i as i64));
            assert_eq!(s.selected_alert_index(), i);
        }
    }

    // ── Selection preserves alert id ─────────────────────────────────────────

    #[test]
    fn select_at_records_alert_id_and_resets_scroll() {
        let mut s = AlertWorkbenchState::new();
        s.right_detail_scroll = 9;
        s.bottom_detail_scroll = 9;
        s.select_at(2, 5, Some(77));
        assert_eq!(s.selected_alert_index, 2);
        assert_eq!(s.selected_alert_id, Some(77));
        assert_eq!(s.right_detail_scroll, 0);
        assert_eq!(s.bottom_detail_scroll, 0);
    }

    #[test]
    fn restore_selection_by_id_finds_preserved_alert() {
        let mut s = AlertWorkbenchState::new();
        s.selected_alert_id = Some(42);
        let ids = [10i64, 20, 42, 30];
        let found = s.restore_selection_by_id(ids.len(), |i| ids.get(i).copied());
        assert!(found);
        assert_eq!(s.selected_alert_index, 2);
    }

    #[test]
    fn restore_selection_falls_back_when_id_missing() {
        let mut s = AlertWorkbenchState::new();
        s.selected_alert_id = Some(999);
        s.selected_alert_index = 5;
        let ids = [10i64, 20, 30];
        let found = s.restore_selection_by_id(ids.len(), |i| ids.get(i).copied());
        assert!(!found);
        // Clamps out-of-range index into bounds.
        assert_eq!(s.selected_alert_index, 2);
    }

    // ── Scroll clamping ──────────────────────────────────────────────────────

    #[test]
    fn clamp_scroll_returns_zero_when_content_fits() {
        assert_eq!(clamp_scroll(5, 10, 20), 0);
        assert_eq!(clamp_scroll(0, 5, 5), 0);
    }

    #[test]
    fn detail_scroll_clamps_at_max() {
        let mut s = AlertWorkbenchState::new();
        // 100 content lines, 10 visible => max scroll 90.
        s.scroll_detail(500, 100, 10);
        assert_eq!(s.right_detail_scroll, 90);
        s.scroll_detail(-500, 100, 10);
        assert_eq!(s.right_detail_scroll, 0);
    }

    #[test]
    fn context_scroll_clamps_at_max() {
        let mut s = AlertWorkbenchState::new();
        s.scroll_context(40, 50, 10);
        assert_eq!(s.bottom_detail_scroll, 40);
        s.scroll_context(40, 50, 10); // would be 80, clamp to 40
        assert_eq!(s.bottom_detail_scroll, 40);
    }

    // ── Filter / sort ────────────────────────────────────────────────────────

    #[test]
    fn filter_is_empty_reflects_knobs() {
        let mut f = AlertFilterState::default();
        assert!(f.is_empty());
        f.text = "ransom".into();
        assert!(!f.is_empty());
        f.text.clear();
        f.severity = Some(Criticality::High);
        assert!(!f.is_empty());
    }

    #[test]
    fn default_sort_is_newest_first() {
        let s = AlertWorkbenchState::new();
        assert_eq!(s.alert_sort, AlertSortMode::NewestFirst);
    }
}

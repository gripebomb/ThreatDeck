use crate::types::{AlertDisposition, AlertListItem, AlertStatus, Criticality};

/// Which pane of the alert workbench is focused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlertPane {
    #[default]
    List,
    Details,
    Context,
}

/// Which context tab is active in the bottom-right pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlertContextTab {
    #[default]
    Iocs,
    Metadata,
    TriageHistory,
}

/// Sort mode for the alert list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlertSortMode {
    #[default]
    NewestFirst,
    OldestFirst,
    SeverityDesc,
    SeverityAsc,
}

/// Filter state for the alert workbench list.
#[derive(Debug, Clone, Default)]
pub struct AlertFilterState {
    pub text: String,
    pub criticality: Option<Criticality>,
    pub status: Option<AlertStatus>,
    pub disposition: Option<AlertDisposition>,
    pub unread_only: bool,
    pub hide_closed: bool,
}

/// Interaction state for the split-pane alert workbench.
///
/// Tracks pane focus, selection index, context tab, filters, sort mode, and
/// scroll offsets. Does NOT hold the loaded AlertWorkbenchBundle — that is
/// managed by the App layer and loaded on selection change.
#[derive(Debug, Clone)]
pub struct AlertWorkbenchState {
    pub alerts: Vec<AlertListItem>,
    pub focused_pane: AlertPane,
    pub active_context_tab: AlertContextTab,
    pub selected_alert_index: usize,
    pub filter: AlertFilterState,
    pub sort_mode: AlertSortMode,
    pub list_scroll: u16,
    pub detail_scroll: u16,
    pub context_scroll: u16,
}

impl AlertWorkbenchState {
    /// Create a new state with empty alert list and default values.
    pub fn new() -> Self {
        Self {
            alerts: Vec::new(),
            focused_pane: AlertPane::default(),
            active_context_tab: AlertContextTab::default(),
            selected_alert_index: 0,
            filter: AlertFilterState::default(),
            sort_mode: AlertSortMode::default(),
            list_scroll: 0,
            detail_scroll: 0,
            context_scroll: 0,
        }
    }

    /// Replace the alert list and clamp selection to valid bounds.
    pub fn set_alerts(&mut self, alerts: Vec<AlertListItem>) {
        self.alerts = alerts;
        if self.alerts.is_empty() {
            self.selected_alert_index = 0;
        } else if self.selected_alert_index >= self.alerts.len() {
            self.selected_alert_index = self.alerts.len() - 1;
        }
    }

    /// Return the ID of the currently selected alert, if any.
    pub fn selected_alert_id(&self) -> Option<i64> {
        self.alerts.get(self.selected_alert_index).map(|a| a.id)
    }

    /// Move selection up by one. Clamps at 0. Resets detail/context scroll on change.
    pub fn move_selection_up(&mut self) {
        if self.selected_alert_index > 0 {
            self.selected_alert_index -= 1;
            self.detail_scroll = 0;
            self.context_scroll = 0;
        }
    }

    /// Move selection down by one. Clamps at last index. Resets detail/context scroll on change.
    pub fn move_selection_down(&mut self) {
        if self.selected_alert_index + 1 < self.alerts.len() {
            self.selected_alert_index += 1;
            self.detail_scroll = 0;
            self.context_scroll = 0;
        }
    }

    /// Cycle focus to the next pane (List -> Details -> Context -> List).
    pub fn focus_next_pane(&mut self) {
        self.focused_pane = match self.focused_pane {
            AlertPane::List => AlertPane::Details,
            AlertPane::Details => AlertPane::Context,
            AlertPane::Context => AlertPane::List,
        };
    }

    /// Cycle focus to the previous pane (List -> Context -> Details -> List).
    pub fn focus_prev_pane(&mut self) {
        self.focused_pane = match self.focused_pane {
            AlertPane::List => AlertPane::Context,
            AlertPane::Details => AlertPane::List,
            AlertPane::Context => AlertPane::Details,
        };
    }

    /// Focus a specific pane.
    pub fn focus_pane(&mut self, pane: AlertPane) {
        self.focused_pane = pane;
    }

    /// Cycle to the next context tab (IOCs -> Metadata -> TriageHistory -> IOCs).
    pub fn cycle_context_tab_next(&mut self) {
        self.active_context_tab = match self.active_context_tab {
            AlertContextTab::Iocs => AlertContextTab::Metadata,
            AlertContextTab::Metadata => AlertContextTab::TriageHistory,
            AlertContextTab::TriageHistory => AlertContextTab::Iocs,
        };
    }

    /// Cycle to the previous context tab (IOCs -> TriageHistory -> Metadata -> IOCs).
    pub fn cycle_context_tab_prev(&mut self) {
        self.active_context_tab = match self.active_context_tab {
            AlertContextTab::Iocs => AlertContextTab::TriageHistory,
            AlertContextTab::Metadata => AlertContextTab::Iocs,
            AlertContextTab::TriageHistory => AlertContextTab::Metadata,
        };
    }

    /// Set a specific context tab.
    pub fn set_context_tab(&mut self, tab: AlertContextTab) {
        self.active_context_tab = tab;
    }

    /// Reset all scroll offsets to zero.
    pub fn reset_scroll(&mut self) {
        self.list_scroll = 0;
        self.detail_scroll = 0;
        self.context_scroll = 0;
    }

    /// Reset only the detail pane scroll. Called on selection change.
    pub fn reset_detail_scroll(&mut self) {
        self.detail_scroll = 0;
    }

    /// Reset only the context pane scroll. Called on selection change.
    pub fn reset_context_scroll(&mut self) {
        self.context_scroll = 0;
    }
}

impl Default for AlertWorkbenchState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AlertDisposition, AlertStatus, Criticality};
    use chrono::Utc;

    fn make_item(id: i64) -> AlertListItem {
        AlertListItem {
            id,
            title: format!("Alert {}", id),
            criticality: Criticality::Medium,
            status: AlertStatus::New,
            disposition: AlertDisposition::Unknown,
            read: false,
            detected_at: Utc::now(),
            feed_name: "Feed".into(),
            keyword_pattern: "pattern".into(),
        }
    }

    #[test]
    fn test_default_state() {
        let s = AlertWorkbenchState::new();
        assert_eq!(s.focused_pane, AlertPane::List);
        assert_eq!(s.active_context_tab, AlertContextTab::Iocs);
        assert_eq!(s.selected_alert_index, 0);
        assert!(s.alerts.is_empty());
        assert_eq!(s.list_scroll, 0);
        assert_eq!(s.detail_scroll, 0);
        assert_eq!(s.context_scroll, 0);
    }

    #[test]
    fn test_default_derived_trait() {
        let s = AlertWorkbenchState::default();
        assert_eq!(s.focused_pane, AlertPane::List);
        assert_eq!(s.active_context_tab, AlertContextTab::Iocs);
        assert_eq!(s.selected_alert_index, 0);
        assert!(s.alerts.is_empty());
        assert_eq!(s.list_scroll, 0);
        assert_eq!(s.detail_scroll, 0);
        assert_eq!(s.context_scroll, 0);
    }

    #[test]
    fn test_selection_clamps_at_top() {
        let mut s = AlertWorkbenchState::new();
        s.set_alerts(vec![make_item(1), make_item(2), make_item(3)]);
        assert_eq!(s.selected_alert_index, 0);
        s.move_selection_up();
        assert_eq!(s.selected_alert_index, 0);
    }

    #[test]
    fn test_selection_clamps_at_bottom() {
        let mut s = AlertWorkbenchState::new();
        s.set_alerts(vec![make_item(1), make_item(2), make_item(3)]);
        s.move_selection_down();
        s.move_selection_down();
        assert_eq!(s.selected_alert_index, 2);
        s.move_selection_down();
        assert_eq!(s.selected_alert_index, 2);
    }

    #[test]
    fn test_selection_move_resets_scroll() {
        let mut s = AlertWorkbenchState::new();
        s.set_alerts(vec![make_item(1), make_item(2), make_item(3)]);
        s.detail_scroll = 10;
        s.context_scroll = 5;
        s.move_selection_down();
        assert_eq!(s.detail_scroll, 0);
        assert_eq!(s.context_scroll, 0);
        assert_eq!(s.selected_alert_index, 1);
    }

    #[test]
    fn test_selection_no_move_keeps_scroll() {
        let mut s = AlertWorkbenchState::new();
        s.set_alerts(vec![make_item(1)]);
        s.detail_scroll = 10;
        s.move_selection_down();
        assert_eq!(s.detail_scroll, 10);
    }

    #[test]
    fn test_set_alerts_clamps_selection() {
        let mut s = AlertWorkbenchState::new();
        s.selected_alert_index = 5;
        s.set_alerts(vec![make_item(1), make_item(2)]);
        assert_eq!(s.selected_alert_index, 1);
    }

    #[test]
    fn test_set_alerts_empty_resets_selection() {
        let mut s = AlertWorkbenchState::new();
        s.set_alerts(Vec::new());
        assert_eq!(s.selected_alert_index, 0);
    }

    #[test]
    fn test_focus_next_cycles() {
        let mut s = AlertWorkbenchState::new();
        assert_eq!(s.focused_pane, AlertPane::List);
        s.focus_next_pane();
        assert_eq!(s.focused_pane, AlertPane::Details);
        s.focus_next_pane();
        assert_eq!(s.focused_pane, AlertPane::Context);
        s.focus_next_pane();
        assert_eq!(s.focused_pane, AlertPane::List);
    }

    #[test]
    fn test_focus_prev_cycles() {
        let mut s = AlertWorkbenchState::new();
        assert_eq!(s.focused_pane, AlertPane::List);
        s.focus_prev_pane();
        assert_eq!(s.focused_pane, AlertPane::Context);
        s.focus_prev_pane();
        assert_eq!(s.focused_pane, AlertPane::Details);
        s.focus_prev_pane();
        assert_eq!(s.focused_pane, AlertPane::List);
    }

    #[test]
    fn test_focus_pane_sets_specific() {
        let mut s = AlertWorkbenchState::new();
        s.focus_pane(AlertPane::Context);
        assert_eq!(s.focused_pane, AlertPane::Context);
    }

    #[test]
    fn test_context_tab_next_cycles() {
        let mut s = AlertWorkbenchState::new();
        assert_eq!(s.active_context_tab, AlertContextTab::Iocs);
        s.cycle_context_tab_next();
        assert_eq!(s.active_context_tab, AlertContextTab::Metadata);
        s.cycle_context_tab_next();
        assert_eq!(s.active_context_tab, AlertContextTab::TriageHistory);
        s.cycle_context_tab_next();
        assert_eq!(s.active_context_tab, AlertContextTab::Iocs);
    }

    #[test]
    fn test_context_tab_prev_cycles() {
        let mut s = AlertWorkbenchState::new();
        assert_eq!(s.active_context_tab, AlertContextTab::Iocs);
        s.cycle_context_tab_prev();
        assert_eq!(s.active_context_tab, AlertContextTab::TriageHistory);
        s.cycle_context_tab_prev();
        assert_eq!(s.active_context_tab, AlertContextTab::Metadata);
        s.cycle_context_tab_prev();
        assert_eq!(s.active_context_tab, AlertContextTab::Iocs);
    }

    #[test]
    fn test_set_context_tab() {
        let mut s = AlertWorkbenchState::new();
        s.set_context_tab(AlertContextTab::TriageHistory);
        assert_eq!(s.active_context_tab, AlertContextTab::TriageHistory);
    }

    #[test]
    fn test_reset_scroll_all() {
        let mut s = AlertWorkbenchState::new();
        s.list_scroll = 3;
        s.detail_scroll = 7;
        s.context_scroll = 9;
        s.reset_scroll();
        assert_eq!(s.list_scroll, 0);
        assert_eq!(s.detail_scroll, 0);
        assert_eq!(s.context_scroll, 0);
    }

    #[test]
    fn test_selected_alert_id() {
        let mut s = AlertWorkbenchState::new();
        s.set_alerts(vec![make_item(10), make_item(20), make_item(30)]);
        assert_eq!(s.selected_alert_id(), Some(10));
        s.move_selection_down();
        assert_eq!(s.selected_alert_id(), Some(20));
        s.set_alerts(Vec::new());
        assert_eq!(s.selected_alert_id(), None);
    }
}

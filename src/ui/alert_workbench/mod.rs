//! Split-pane alert workbench: the live `Screen::Alerts` view.
//!
//! This module groups everything behind the three-pane alert workbench:
//! - [`state`] — pane focus, context tabs, selection, scroll, filters.
//! - [`view_models`] — TUI view models and the selected-alert data bundle.
//! - [`layout`] — wide/narrow/tiny terminal layout engine (internal).
//! - [`alert_list`] / [`alert_details`] / [`context_tabs`] — pane renderers.
//! - [`page`] — assembles the panes and wires keyboard navigation.
//! - [`triage`] — analyst triage/export actions.
//!
//! See `docs/MASTER_PLAN.md` and `docs/ARCHITECTURE.md`.

mod layout;

pub mod alert_details;
pub mod alert_list;
pub mod context_tabs;
pub mod page;
pub mod state;
pub mod triage;
pub mod view_models;

// Re-export only the items consumed outside this module via the
// `alert_workbench::<name>` path. Submodule types that are only used internally
// (e.g. `layout::classify`, `state::clamp_scroll`) stay module-private.
pub use layout::{compute_layout, LayoutMode};
pub use state::{AlertContextTab, AlertFilterState, AlertPane, AlertWorkbenchState};
pub use view_models::{
    AlertDetailViewModel, AlertListItem, AlertWorkbenchBundle, IndicatorViewModel,
    TriageEventViewModel,
};

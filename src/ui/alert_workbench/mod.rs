//! Split-pane alert workbench foundation.
//!
//! Phase 1 delivers non-visual building blocks only:
//! - [`state`] — pane focus, context tabs, selection, scroll, filters, loading.
//! - [`view_models`] — TUI view models and the selected-alert data bundle.
//! - [`layout`] — wide/narrow/tiny terminal layout engine.
//!
//! Rendering, event wiring, and analyst actions land in later phases. See
//! `docs/MASTER_PLAN.md` (Phase 1) and `docs/ARCHITECTURE.md`.

// Phase 1 ships non-visual foundation; the re-exported items below are wired
// into rendering in Phase 2+. Suppress the interim unused-import noise locally
// (mirrors the crate-level `#![allow(dead_code)]` stance).
#![allow(unused_imports)]

pub mod alert_details;
pub mod alert_list;
pub mod context_tabs;
pub mod layout;
pub mod page;
pub mod state;
pub mod triage;
pub mod view_models;

pub use layout::{classify, compute_layout, compute_layout_from_size, LayoutMode, WorkbenchLayout};
pub use state::{
    clamp_scroll, AlertContextTab, AlertFilterState, AlertPane, AlertSortMode, AlertWorkbenchState,
};
pub use view_models::{
    AlertDetailViewModel, AlertListItem, AlertWorkbenchBundle, EnrichmentViewModel,
    IndicatorViewModel, TriageEventViewModel,
};

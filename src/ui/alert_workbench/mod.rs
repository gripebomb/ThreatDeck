//! Split-pane alert workbench: the live `Screen::Alerts` view.
//!
//! Public entry points are [`page`] (rendering + keyboard wiring) and
//! [`triage`] (analyst actions). Everything else — [`state`], [`view_models`],
//! [`layout`], and the per-pane renderers ([`alert_list`] / [`alert_details`] /
//! [`context_tabs`]) — is internal and reached only through the type re-exports
//! below, so callers outside this module can't depend on renderer internals.
//!
//! See `docs/MASTER_PLAN.md` and `docs/ARCHITECTURE.md`.

mod alert_details;
mod alert_list;
mod context_tabs;
mod layout;
mod state;
mod view_models;

pub mod page;
pub mod triage;

// Re-export the items consumed outside this module. Internal-only items (e.g.
// `layout::classify`, `state::clamp_scroll`, the renderer functions) stay
// module-private behind this surface.
pub use layout::{compute_layout, LayoutMode};
pub use state::{AlertContextTab, AlertFilterState, AlertPane, AlertWorkbenchState};
pub use view_models::{
    AlertDetailViewModel, AlertListItem, AlertWorkbenchBundle, EnrichmentViewModel,
    IndicatorViewModel, TriageEventViewModel,
};

//! Split-pane layout calculation with safe terminal fallbacks.
//!
//! Pure geometry — no rendering, no terminal access, no panics. Wide terminals
//! get the full three-pane split; narrow terminals collapse to a single focused
//! pane; tiny terminals never panic and yield a safe minimal rect.
//!
//! See `tickets/03-layout-engine.md` and `docs/ACCEPTANCE_CRITERIA.md`
//! (Minimum Terminal Behavior).

use ratatui::layout::Rect;

use crate::ui::alert_workbench::state::AlertPane;

/// Minimum terminal size for the full wide split-pane layout.
///
/// From `docs/ACCEPTANCE_CRITERIA.md`: width >= 110 and height >= 30 uses the
/// full split-pane layout.
pub const WIDE_MIN_WIDTH: u16 = 110;
pub const WIDE_MIN_HEIGHT: u16 = 30;

/// Below this width the layout collapses to single-pane mode.
pub const NARROW_MAX_WIDTH: u16 = 99;

/// Below either of these the terminal is too small to be useful; the layout
/// returns a single safe rect and never panics.
pub const TINY_MIN_WIDTH: u16 = 20;
pub const TINY_MIN_HEIGHT: u16 = 8;

/// Left (alert list) pane takes this share of the width on wide terminals.
/// From `tickets/03-layout-engine.md`: left 35%, right 65%.
pub const LEFT_WIDTH_PCT: u32 = 35;

/// Which layout was chosen for the current terminal size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    /// Full split: alert list + details + context panel.
    Wide,
    /// Single focused pane (narrow width or short height).
    Narrow,
    /// Terminal too small to be useful; render a compact message only.
    Tiny,
}

/// Result of layout calculation: the three pane rectangles plus the chosen mode.
///
/// In [`LayoutMode::Narrow`] / [`LayoutMode::Tiny`], only the focused pane
/// receives a usable rectangle; the others are zero-sized placeholders so the
/// renderer knows not to draw them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkbenchLayout {
    pub mode: LayoutMode,
    pub full: Rect,
    pub alert_list: Rect,
    pub alert_details: Rect,
    pub context_panel: Rect,
}

impl WorkbenchLayout {
    /// True when the wide split-pane layout is active.
    pub fn is_wide(&self) -> bool {
        self.mode == LayoutMode::Wide
    }

    /// The single visible pane in narrow/tiny mode.
    pub fn active_pane_rect(&self, pane: AlertPane) -> Rect {
        match pane {
            AlertPane::AlertList => self.alert_list,
            AlertPane::AlertDetails => self.alert_details,
            AlertPane::ContextPanel => self.context_panel,
        }
    }
}

/// Classify the terminal size into a layout mode.
pub fn classify(width: u16, height: u16) -> LayoutMode {
    if width < TINY_MIN_WIDTH || height < TINY_MIN_HEIGHT {
        LayoutMode::Tiny
    } else if width >= WIDE_MIN_WIDTH && height >= WIDE_MIN_HEIGHT {
        LayoutMode::Wide
    } else {
        LayoutMode::Narrow
    }
}

/// Compute the workbench layout for a given content rectangle and focused pane.
///
/// `area` is the content region available to the workbench (i.e. after the nav
/// tab bar, title and status rows have been reserved by the caller). The
/// function never panics: all arithmetic is saturating.
pub fn compute_layout(area: Rect, focused: AlertPane) -> WorkbenchLayout {
    let mode = classify(area.width, area.height);
    match mode {
        LayoutMode::Wide => wide_layout(area),
        LayoutMode::Narrow | LayoutMode::Tiny => single_pane_layout(area, focused, mode),
    }
}

/// Convenience wrapper taking raw dimensions (origin at 0,0).
pub fn compute_layout_from_size(width: u16, height: u16, focused: AlertPane) -> WorkbenchLayout {
    compute_layout(Rect::new(0, 0, width, height), focused)
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn wide_layout(area: Rect) -> WorkbenchLayout {
    // Left pane = 35% of width; right column = the remainder.
    let left_w = ((area.width as u32 * LEFT_WIDTH_PCT) / 100) as u16;
    let left_w = left_w.max(1);
    let right_w = area.width.saturating_sub(left_w);

    let left = safe_rect(area.x, area.y, left_w, area.height);
    let right_x = area.x.saturating_add(left_w);

    // Right column split vertically 50/50 into details (top) and context (bottom).
    let half_h = area.height / 2;
    let details = safe_rect(right_x, area.y, right_w, half_h);
    let context_y = area.y.saturating_add(half_h);
    let context_h = area.height.saturating_sub(half_h);
    let context = safe_rect(right_x, context_y, right_w, context_h);

    WorkbenchLayout {
        mode: LayoutMode::Wide,
        full: area,
        alert_list: left,
        alert_details: details,
        context_panel: context,
    }
}

/// Narrow/tiny: give the entire area to the focused pane; others zero-sized.
fn single_pane_layout(area: Rect, focused: AlertPane, mode: LayoutMode) -> WorkbenchLayout {
    let empty = Rect::ZERO;
    let (alert_list, details, context) = match focused {
        AlertPane::AlertList => (area, empty, empty),
        AlertPane::AlertDetails => (empty, area, empty),
        AlertPane::ContextPanel => (empty, empty, area),
    };
    WorkbenchLayout {
        mode,
        full: area,
        alert_list,
        alert_details: details,
        context_panel: context,
    }
}

/// Build a rect from already-clamped coordinates. Callers guarantee the
/// values are non-negative and within the usable area via saturating math.
fn safe_rect(x: u16, y: u16, width: u16, height: u16) -> Rect {
    Rect::new(x, y, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Wide layout ──────────────────────────────────────────────────────────

    #[test]
    fn wide_terminal_returns_three_pane_rectangles() {
        let layout = compute_layout_from_size(140, 40, AlertPane::AlertList);
        assert_eq!(layout.mode, LayoutMode::Wide);
        assert!(layout.is_wide());
        assert!(layout.alert_list.width > 0);
        assert!(layout.alert_details.width > 0);
        assert!(layout.context_panel.width > 0);
        assert!(layout.alert_list.height > 0);
        assert!(layout.alert_details.height > 0);
        assert!(layout.context_panel.height > 0);

        // Left pane ~35% of width.
        assert_eq!(layout.alert_list.width, 140 * 35 / 100);

        // Left + right column widths sum to the full width.
        assert_eq!(layout.alert_list.width + layout.alert_details.width, 140);

        // Right column split 50/50 vertically.
        assert_eq!(layout.alert_details.height, 40 / 2);
        assert_eq!(layout.context_panel.height, 40 - 40 / 2);

        // Right panes share x and width.
        assert_eq!(layout.alert_details.x, layout.context_panel.x);
        assert_eq!(layout.alert_details.width, layout.context_panel.width);

        // Context sits directly below details.
        assert_eq!(
            layout.context_panel.y,
            layout.alert_details.y + layout.alert_details.height
        );
    }

    #[test]
    fn wide_threshold_at_minimum_size() {
        // Exactly 110x30 should still be wide.
        let layout =
            compute_layout_from_size(WIDE_MIN_WIDTH, WIDE_MIN_HEIGHT, AlertPane::AlertList);
        assert_eq!(layout.mode, LayoutMode::Wide);
    }

    // ── Narrow fallback ───────────────────────────────────────────────────────

    #[test]
    fn narrow_terminal_uses_single_pane_fallback() {
        let layout = compute_layout_from_size(80, 30, AlertPane::AlertList);
        assert_eq!(layout.mode, LayoutMode::Narrow);
        assert!(!layout.is_wide());

        // Focused (AlertList) gets the full area; others are zero.
        assert_eq!(layout.alert_list, layout.full);
        assert_eq!(layout.alert_details, Rect::ZERO);
        assert_eq!(layout.context_panel, Rect::ZERO);
    }

    #[test]
    fn narrow_fallback_follows_focused_pane() {
        let wide = compute_layout_from_size(90, 30, AlertPane::AlertList);
        assert_eq!(wide.alert_list, wide.full);
        assert_eq!(wide.alert_details, Rect::ZERO);

        let details = compute_layout_from_size(90, 30, AlertPane::AlertDetails);
        assert_eq!(details.alert_details, details.full);
        assert_eq!(details.alert_list, Rect::ZERO);

        let ctx = compute_layout_from_size(90, 30, AlertPane::ContextPanel);
        assert_eq!(ctx.context_panel, ctx.full);
        assert_eq!(ctx.alert_list, Rect::ZERO);
    }

    #[test]
    fn short_height_uses_single_pane_fallback() {
        // Wide width but short height => narrow.
        let layout = compute_layout_from_size(120, 20, AlertPane::AlertList);
        assert_eq!(layout.mode, LayoutMode::Narrow);
        assert_eq!(layout.alert_list, layout.full);
    }

    // ── Tiny terminal safety ─────────────────────────────────────────────────

    #[test]
    fn tiny_width_does_not_panic() {
        let layout = compute_layout_from_size(5, 30, AlertPane::AlertList);
        assert_eq!(layout.mode, LayoutMode::Tiny);
        // Active pane still maps to full area; no panic, no overflow.
        assert_eq!(layout.alert_list, layout.full);
    }

    #[test]
    fn tiny_height_does_not_panic() {
        let layout = compute_layout_from_size(120, 3, AlertPane::AlertList);
        assert_eq!(layout.mode, LayoutMode::Tiny);
        assert_eq!(layout.alert_list, layout.full);
    }

    #[test]
    fn zero_size_terminal_does_not_panic() {
        let layout = compute_layout_from_size(0, 0, AlertPane::AlertList);
        assert_eq!(layout.mode, LayoutMode::Tiny);
        assert_eq!(layout.full, Rect::ZERO);
    }

    #[test]
    fn extremely_large_terminal_does_not_panic_or_overflow() {
        // u16::MAX would overflow naive width*35/100 math; ensure it doesn't.
        let layout = compute_layout_from_size(u16::MAX, u16::MAX, AlertPane::AlertList);
        assert_eq!(layout.mode, LayoutMode::Wide);
        // Left + right column should still partition the full width cleanly.
        assert_eq!(
            layout.alert_list.width + layout.alert_details.width,
            u16::MAX
        );
    }

    // ── Classification ────────────────────────────────────────────────────────

    #[test]
    fn classify_respects_thresholds() {
        assert_eq!(classify(140, 40), LayoutMode::Wide);
        assert_eq!(classify(WIDE_MIN_WIDTH, WIDE_MIN_HEIGHT), LayoutMode::Wide);
        assert_eq!(classify(80, 30), LayoutMode::Narrow);
        assert_eq!(classify(120, 20), LayoutMode::Narrow);
        assert_eq!(classify(15, 30), LayoutMode::Tiny);
        assert_eq!(classify(120, 5), LayoutMode::Tiny);
        assert_eq!(classify(0, 0), LayoutMode::Tiny);
    }
}

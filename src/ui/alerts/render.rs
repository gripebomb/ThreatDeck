use crate::app::App;
use crate::ui::alerts::alert_details::draw_alert_details;
use crate::ui::alerts::alert_list::draw_alert_list;
use crate::ui::alerts::context_tabs::draw_context_panel;
use crate::ui::alerts::layout::{calculate_layout, AlertWorkbenchLayout};
use crate::ui::alerts::state::AlertPane;
use ratatui::{layout::Rect, Frame};

/// Render the split-pane alert workbench into the given area.
///
/// Wide terminals get a three-pane split (list left, details top-right,
/// context bottom-right). Narrow terminals get a single list pane.
/// Tiny/zero-size areas return safely without rendering.
pub fn draw_workbench(f: &mut Frame, app: &App, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let focused_pane = app.workbench_state.focused_pane;

    match calculate_layout(area) {
        AlertWorkbenchLayout::Split {
            list,
            details,
            context,
        } => {
            draw_alert_list(f, app, list, focused_pane == AlertPane::List);
            draw_alert_details(f, app, details, focused_pane == AlertPane::Details);
            draw_context_panel(f, app, context, focused_pane == AlertPane::Context);
        }
        AlertWorkbenchLayout::Single { main } => {
            // Narrow terminal: show just the list pane, always focused.
            draw_alert_list(f, app, main, true);
        }
    }
}

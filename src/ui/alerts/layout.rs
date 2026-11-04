use ratatui::layout::Rect;

/// Layout result for the alert workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertWorkbenchLayout {
    /// Wide terminal: three-pane split (list left, details top-right, context bottom-right).
    Split {
        list: Rect,
        details: Rect,
        context: Rect,
    },
    /// Narrow terminal: single full-area pane.
    Single { main: Rect },
}

/// Minimum terminal dimensions for the wide split-pane layout.
const WIDE_MIN_WIDTH: u16 = 100;
const WIDE_MIN_HEIGHT: u16 = 24;

/// Calculate the alert workbench layout for the given terminal area.
///
/// Wide terminals (>= 100x24) get a three-pane split:
///   - Left pane: 35% width (alert list)
///   - Right pane: 65% width, split 50/50 vertically (details top, context bottom)
///
/// Narrow terminals get a single full-area pane.
/// Tiny terminals never panic — they fall through to the Single variant.
pub fn calculate_layout(area: Rect) -> AlertWorkbenchLayout {
    if area.width >= WIDE_MIN_WIDTH && area.height >= WIDE_MIN_HEIGHT {
        use ratatui::layout::{Constraint, Direction, Layout};

        let main = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
            .split(area);

        let right = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(main[1]);

        AlertWorkbenchLayout::Split {
            list: main[0],
            details: right[0],
            context: right[1],
        }
    } else {
        AlertWorkbenchLayout::Single { main: area }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wide_layout_returns_split() {
        let area = Rect::new(0, 0, 120, 30);
        let layout = calculate_layout(area);

        match layout {
            AlertWorkbenchLayout::Split {
                list,
                details,
                context,
            } => {
                // Left pane is ~35% of 120 == 42 (allow rounding slack).
                assert!(
                    list.width >= 40 && list.width <= 44,
                    "list.width={}",
                    list.width
                );
                assert!(details.area() > 0, "details has zero area");
                assert!(context.area() > 0, "context has zero area");
                assert_eq!(list.x, 0);
                assert_eq!(list.y, 0);
                assert!(details.x > list.x, "details should be right of list");
                assert!(context.y > details.y, "context should be below details");
            }
            AlertWorkbenchLayout::Single { .. } => panic!("expected Split for wide terminal"),
        }
    }

    #[test]
    fn test_wide_layout_exact_threshold() {
        let area = Rect::new(0, 0, 100, 24);
        assert!(matches!(
            calculate_layout(area),
            AlertWorkbenchLayout::Split { .. }
        ));
    }

    #[test]
    fn test_narrow_width_returns_single() {
        let area = Rect::new(0, 0, 80, 30);
        match calculate_layout(area) {
            AlertWorkbenchLayout::Single { main } => assert_eq!(main, area),
            _ => panic!("expected Single for narrow width"),
        }
    }

    #[test]
    fn test_narrow_height_returns_single() {
        let area = Rect::new(0, 0, 120, 20);
        assert!(matches!(
            calculate_layout(area),
            AlertWorkbenchLayout::Single { .. }
        ));
    }

    #[test]
    fn test_tiny_terminal_does_not_panic() {
        let area = Rect::new(0, 0, 1, 1);
        assert!(matches!(
            calculate_layout(area),
            AlertWorkbenchLayout::Single { .. }
        ));
    }

    #[test]
    fn test_zero_size_does_not_panic() {
        let area = Rect::new(0, 0, 0, 0);
        assert!(matches!(
            calculate_layout(area),
            AlertWorkbenchLayout::Single { .. }
        ));
    }

    #[test]
    fn test_split_panes_do_not_overlap() {
        let area = Rect::new(0, 0, 120, 30);
        match calculate_layout(area) {
            AlertWorkbenchLayout::Split {
                list,
                details,
                context,
            } => {
                assert!(
                    list.x + list.width <= details.x,
                    "list overlaps details horizontally"
                );
                assert!(
                    details.y + details.height <= context.y,
                    "details overlaps context vertically"
                );
            }
            _ => panic!("expected Split"),
        }
    }

    #[test]
    fn test_split_panes_fill_area() {
        let area = Rect::new(0, 0, 120, 30);
        match calculate_layout(area) {
            AlertWorkbenchLayout::Split {
                list,
                details,
                context,
            } => {
                assert_eq!(list.height, 30, "list should span full height");
                assert!(
                    (details.width + list.width) as i32 - 120 <= 1,
                    "list+details widths should fill 120"
                );
                assert!(
                    (details.height + context.height) as i32 - 30 <= 1,
                    "details+context heights should fill 30"
                );
            }
            _ => panic!("expected Split"),
        }
    }
}

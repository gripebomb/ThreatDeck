use crate::types::Criticality;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Modifier, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListMotion {
    Up,
    Down,
    Top,
    Bottom,
    HalfPageUp(usize),
    HalfPageDown(usize),
}

pub fn move_selection(selected: usize, len: usize, motion: ListMotion) -> usize {
    if len == 0 {
        return 0;
    }

    match motion {
        ListMotion::Up => selected.saturating_sub(1),
        ListMotion::Down => selected.saturating_add(1).min(len - 1),
        ListMotion::Top => 0,
        ListMotion::Bottom => len - 1,
        ListMotion::HalfPageUp(amount) => selected.saturating_sub(amount.max(1)),
        ListMotion::HalfPageDown(amount) => selected.saturating_add(amount.max(1)).min(len - 1),
    }
}

pub fn criticality_label(criticality: Criticality) -> &'static str {
    match criticality {
        Criticality::Low => "L Low",
        Criticality::Medium => "M Medium",
        Criticality::High => "H High",
        Criticality::Critical => "C Critical",
    }
}

pub fn selected_style() -> Style {
    Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
}

pub fn motion_from_key(key: KeyEvent, pending_g: &mut bool) -> Option<ListMotion> {
    if *pending_g {
        *pending_g = false;
        if key.code == KeyCode::Char('g') {
            return Some(ListMotion::Top);
        }
    }

    match key.code {
        KeyCode::Down | KeyCode::Char('j') => Some(ListMotion::Down),
        KeyCode::Up | KeyCode::Char('k') => Some(ListMotion::Up),
        KeyCode::Char('G') => Some(ListMotion::Bottom),
        KeyCode::Char('g') => {
            *pending_g = true;
            None
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(ListMotion::HalfPageDown(10))
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(ListMotion::HalfPageUp(10))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_motion_handles_bounds_and_half_pages() {
        assert_eq!(move_selection(0, 0, ListMotion::Down), 0);
        assert_eq!(move_selection(0, 3, ListMotion::Up), 0);
        assert_eq!(move_selection(2, 3, ListMotion::Down), 2);
        assert_eq!(move_selection(7, 10, ListMotion::HalfPageUp(4)), 3);
        assert_eq!(move_selection(7, 10, ListMotion::HalfPageDown(4)), 9);
        assert_eq!(move_selection(5, 10, ListMotion::Top), 0);
        assert_eq!(move_selection(5, 10, ListMotion::Bottom), 9);
    }

    #[test]
    fn criticality_label_includes_monochrome_prefix() {
        assert_eq!(criticality_label(Criticality::Low), "L Low");
        assert_eq!(criticality_label(Criticality::Medium), "M Medium");
        assert_eq!(criticality_label(Criticality::High), "H High");
        assert_eq!(criticality_label(Criticality::Critical), "C Critical");
    }
}

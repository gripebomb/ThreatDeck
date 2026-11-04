use crate::ui::command_palette::command::CommandAvailability;
use crate::ui::command_palette::registry::CommandContext;
use crate::ui::command_palette::{matcher, registry, CommandId, CommandPaletteState, PaletteMode};
use std::collections::HashSet;

#[test]
fn all_command_ids_are_unique() {
    let mut seen = HashSet::new();
    for cmd in registry::ALL_COMMANDS {
        assert!(seen.insert(cmd.id), "Duplicate CommandId: {:?}", cmd.id);
    }
}

#[test]
fn all_canonical_names_are_unique() {
    let mut seen = HashSet::new();
    for cmd in registry::ALL_COMMANDS {
        assert!(
            seen.insert(cmd.canonical),
            "Duplicate canonical name: {}",
            cmd.canonical
        );
    }
}

#[test]
fn all_commands_have_titles() {
    for cmd in registry::ALL_COMMANDS {
        assert!(
            !cmd.title.is_empty(),
            "Command {:?} has empty title",
            cmd.id
        );
    }
}

#[test]
fn all_commands_have_descriptions() {
    for cmd in registry::ALL_COMMANDS {
        assert!(
            !cmd.description.is_empty(),
            "Command {:?} has empty description",
            cmd.id
        );
    }
}

#[test]
fn contextual_commands_filtered_correctly() {
    let ctx_no_selection = CommandContext::default();
    let available = registry::available_commands(&ctx_no_selection);
    for cmd in &available {
        assert!(
            !matches!(
                cmd.availability,
                CommandAvailability::WhenFeedSelected
                    | CommandAvailability::WhenAlertSelected
                    | CommandAvailability::WhenKeywordSelected
            ),
            "Contextual command {:?} should be hidden when nothing selected",
            cmd.id
        );
    }

    let ctx_with_feed = CommandContext {
        has_selected_feed: true,
        ..Default::default()
    };
    let available_with_feed = registry::available_commands(&ctx_with_feed);
    assert!(
        available_with_feed
            .iter()
            .any(|c| c.id == CommandId::FeedEditSelected),
        "FeedEditSelected should be available when feed is selected"
    );
}

#[test]
fn matcher_hides_selection_commands_without_selection() {
    let results = matcher::match_commands("alert mark read", &CommandContext::default());
    assert!(results
        .iter()
        .all(|result| result.command.id != CommandId::AlertMarkSelectedRead));
}

#[test]
fn workbench_commands_require_alerts_screen() {
    let context = CommandContext {
        has_selected_alert: true,
        ..CommandContext::default()
    };
    let focus = matcher::match_commands("alert focus details", &context);
    let triage = matcher::match_commands("alert triage acknowledge", &context);
    assert!(focus.is_empty());
    assert!(triage.is_empty());
}

#[test]
fn normalize_strips_colon() {
    assert_eq!(
        matcher::normalize_input(":feed check all"),
        "feed check all"
    );
}

#[test]
fn normalize_collapses_whitespace() {
    assert_eq!(matcher::normalize_input("  feed   add  "), "feed add");
}

#[test]
fn normalize_lowercases() {
    assert_eq!(matcher::normalize_input("Feed ADD"), "feed add");
}

#[test]
fn empty_query_returns_all_available_commands() {
    let context = CommandContext::default();
    let results = matcher::match_commands("", &context);
    assert_eq!(results.len(), registry::available_commands(&context).len());
}

#[test]
fn search_by_canonical() {
    let results = matcher::match_commands("feed check all", &CommandContext::default());
    assert!(
        results
            .iter()
            .any(|m| m.command.id == CommandId::FeedCheckAll),
        "Should find FeedCheckAll"
    );
}

#[test]
fn search_by_alias() {
    let results = matcher::match_commands("refresh feeds", &CommandContext::default());
    assert!(
        results
            .iter()
            .any(|m| m.command.id == CommandId::FeedCheckAll),
        "Should find FeedCheckAll via alias"
    );
}

#[test]
fn search_by_keyword() {
    let results = matcher::match_commands("sources", &CommandContext::default());
    assert!(
        results.iter().any(|m| m.command.id == CommandId::OpenFeeds),
        "Should find OpenFeeds via keyword"
    );
}

#[test]
fn exact_canonical_ranks_first() {
    let results = matcher::match_commands("feed check all", &CommandContext::default());
    assert_eq!(results[0].command.id, CommandId::FeedCheckAll);
}

#[test]
fn case_insensitive_matching() {
    let lower = matcher::match_commands("feed check all", &CommandContext::default());
    let upper = matcher::match_commands("FEED CHECK ALL", &CommandContext::default());
    assert_eq!(lower.len(), upper.len());
    assert_eq!(lower[0].command.id, upper[0].command.id);
}

#[test]
fn no_match_returns_empty() {
    let results = matcher::match_commands("xyz nonexistent", &CommandContext::default());
    assert!(results.is_empty());
}

#[test]
fn multi_token_match() {
    let context = CommandContext {
        discord_configured: true,
        ..CommandContext::default()
    };
    let results = matcher::match_commands("discord test", &context);
    assert!(
        results
            .iter()
            .any(|m| m.command.id == CommandId::NotifyTestDiscord),
        "Should find NotifyTestDiscord"
    );
}

#[test]
fn open_fuzzy_clears_input() {
    let mut state = CommandPaletteState::default();
    state.input = "previous".to_string();
    state.open_fuzzy(&CommandContext::default());
    assert!(state.is_open);
    assert_eq!(state.mode, PaletteMode::Fuzzy);
    assert_eq!(state.input, "");
}

#[test]
fn open_colon_prefills_colon() {
    let mut state = CommandPaletteState::default();
    state.open_colon(&CommandContext::default());
    assert!(state.is_open);
    assert_eq!(state.mode, PaletteMode::Colon);
    assert_eq!(state.input, ":");
}

#[test]
fn backspace_in_colon_mode_preserves_colon() {
    let mut state = CommandPaletteState::default();
    state.open_colon(&CommandContext::default());
    state.backspace();
    assert_eq!(state.input, ":");
}

#[test]
fn move_down_clamps_at_bottom() {
    let mut state = CommandPaletteState::default();
    state.open_fuzzy(&CommandContext::default());
    let count = state.results.len();
    for _ in 0..count + 5 {
        state.move_down();
    }
    assert_eq!(state.selected_index, count.saturating_sub(1));
}

#[test]
fn move_up_clamps_at_top() {
    let mut state = CommandPaletteState::default();
    state.open_fuzzy(&CommandContext::default());
    state.move_down();
    state.move_down();
    state.move_up();
    state.move_up();
    state.move_up();
    assert_eq!(state.selected_index, 0);
}

#[test]
fn close_resets_state() {
    let mut state = CommandPaletteState::default();
    state.open_fuzzy(&CommandContext::default());
    state.input_char('t');
    state.move_down();
    state.close();
    assert!(!state.is_open);
    assert_eq!(state.input, "");
    assert_eq!(state.selected_index, 0);
    assert!(state.results.is_empty());
}

#[test]
fn selection_valid_after_filtering() {
    let mut state = CommandPaletteState::default();
    state.open_fuzzy(&CommandContext::default());
    state.move_down();
    state.move_down();
    state.input_char('z');
    state.input_char('z');
    state.input_char('z');
    assert_eq!(state.selected_index, 0);
}

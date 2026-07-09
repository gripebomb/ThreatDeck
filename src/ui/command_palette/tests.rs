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
fn empty_query_returns_all_commands() {
    let results = matcher::match_commands("");
    assert_eq!(results.len(), registry::ALL_COMMANDS.len());
}

#[test]
fn workbench_commands_are_searchable() {
    // Pane focus via the palette (no 1/2/3 key binding exists).
    let focus_cases: &[(&str, CommandId)] = &[
        ("focus list", CommandId::AlertFocusList),
        ("focus details", CommandId::AlertFocusDetails),
        ("focus context", CommandId::AlertFocusContext),
    ];
    // Context tabs.
    let tab_cases: &[(&str, CommandId)] = &[
        ("tab indicators", CommandId::AlertTabIndicators),
        ("tab metadata", CommandId::AlertTabMetadata),
        ("tab enrichment", CommandId::AlertTabEnrichment),
        ("tab history", CommandId::AlertTabHistory),
        ("tab raw", CommandId::AlertTabRaw),
    ];
    // One-shot triage.
    let triage_cases: &[(&str, CommandId)] = &[
        ("acknowledge", CommandId::AlertAcknowledge),
        ("investigate", CommandId::AlertInvestigate),
        ("escalate", CommandId::AlertEscalate),
        ("close", CommandId::AlertClose),
        ("reopen", CommandId::AlertReopen),
    ];
    for (query, expected) in focus_cases.iter().chain(tab_cases).chain(triage_cases) {
        let results = matcher::match_commands(query);
        assert!(
            results.iter().any(|m| m.command.id == *expected),
            "query {query:?} should find {expected:?}"
        );
    }
}

#[test]
fn search_by_canonical() {
    let results = matcher::match_commands("feed check all");
    assert!(
        results
            .iter()
            .any(|m| m.command.id == CommandId::FeedCheckAll),
        "Should find FeedCheckAll"
    );
}

#[test]
fn search_by_alias() {
    let results = matcher::match_commands("refresh feeds");
    assert!(
        results
            .iter()
            .any(|m| m.command.id == CommandId::FeedCheckAll),
        "Should find FeedCheckAll via alias"
    );
}

#[test]
fn search_by_keyword() {
    let results = matcher::match_commands("sources");
    assert!(
        results.iter().any(|m| m.command.id == CommandId::OpenFeeds),
        "Should find OpenFeeds via keyword"
    );
}

#[test]
fn exact_canonical_ranks_first() {
    let results = matcher::match_commands("feed check all");
    assert_eq!(results[0].command.id, CommandId::FeedCheckAll);
}

#[test]
fn case_insensitive_matching() {
    let lower = matcher::match_commands("feed check all");
    let upper = matcher::match_commands("FEED CHECK ALL");
    assert_eq!(lower.len(), upper.len());
    assert_eq!(lower[0].command.id, upper[0].command.id);
}

#[test]
fn no_match_returns_empty() {
    let results = matcher::match_commands("xyz nonexistent");
    assert!(results.is_empty());
}

#[test]
fn multi_token_match() {
    let results = matcher::match_commands("discord test");
    assert!(
        results
            .iter()
            .any(|m| m.command.id == CommandId::NotifyTestDiscord),
        "Should find NotifyTestDiscord"
    );
}

#[test]
fn open_fuzzy_clears_input() {
    let mut state = CommandPaletteState {
        input: "previous".to_string(),
        ..CommandPaletteState::default()
    };
    state.open_fuzzy();
    assert!(state.is_open);
    assert_eq!(state.mode, PaletteMode::Fuzzy);
    assert_eq!(state.input, "");
}

#[test]
fn open_colon_prefills_colon() {
    let mut state = CommandPaletteState::default();
    state.open_colon();
    assert!(state.is_open);
    assert_eq!(state.mode, PaletteMode::Colon);
    assert_eq!(state.input, ":");
}

#[test]
fn backspace_in_colon_mode_preserves_colon() {
    let mut state = CommandPaletteState::default();
    state.open_colon();
    state.backspace();
    assert_eq!(state.input, ":");
}

#[test]
fn move_down_clamps_at_bottom() {
    let mut state = CommandPaletteState::default();
    state.open_fuzzy();
    let count = state.results.len();
    for _ in 0..count + 5 {
        state.move_down();
    }
    assert_eq!(state.selected_index, count.saturating_sub(1));
}

#[test]
fn move_up_clamps_at_top() {
    let mut state = CommandPaletteState::default();
    state.open_fuzzy();
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
    state.open_fuzzy();
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
    state.open_fuzzy();
    state.move_down();
    state.move_down();
    state.input_char('z');
    state.input_char('z');
    state.input_char('z');
    assert_eq!(state.selected_index, 0);
}

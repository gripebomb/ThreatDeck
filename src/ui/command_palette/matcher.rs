use super::command::Command;
use super::registry::ALL_COMMANDS;

#[derive(Debug, Clone)]
pub struct CommandMatch {
    pub command: Command,
    pub score: i64,
}

pub fn normalize_input(input: &str) -> String {
    let trimmed = input.trim();
    let without_colon = trimmed.strip_prefix(':').unwrap_or(trimmed);
    let mut result = String::with_capacity(without_colon.len());
    let mut prev_was_space = true;
    for ch in without_colon.chars() {
        if ch.is_whitespace() {
            if !prev_was_space {
                result.push(' ');
                prev_was_space = true;
            }
        } else {
            result.push(ch.to_ascii_lowercase());
            prev_was_space = false;
        }
    }
    if result.ends_with(' ') {
        result.pop();
    }
    result
}

pub fn tokenize(input: &str) -> Vec<&str> {
    input.split_whitespace().collect()
}

pub fn match_commands(input: &str) -> Vec<CommandMatch> {
    let normalized = normalize_input(input);
    if normalized.is_empty() {
        return ALL_COMMANDS
            .iter()
            .map(|cmd| CommandMatch { command: cmd.clone(), score: 0 })
            .collect();
    }

    let tokens = tokenize(&normalized);
    let mut matches: Vec<CommandMatch> = ALL_COMMANDS
        .iter()
        .filter_map(|cmd| score_command(cmd, &tokens, &normalized))
        .collect();

    matches.sort_by(|a, b| {
        b.score.cmp(&a.score)
            .then_with(|| a.command.canonical.len().cmp(&b.command.canonical.len()))
    });

    matches
}

fn score_command(cmd: &Command, tokens: &[&str], normalized: &str) -> Option<CommandMatch> {
    let canonical_lower = cmd.canonical.to_ascii_lowercase();
    let title_lower = cmd.title.to_ascii_lowercase();
    let desc_lower = cmd.description.to_ascii_lowercase();
    let group_lower = cmd.group.label().to_ascii_lowercase();

    if !tokens.iter().all(|tok| {
        canonical_lower.contains(*tok)
            || title_lower.contains(*tok)
            || desc_lower.contains(*tok)
            || group_lower.contains(*tok)
            || cmd.aliases.iter().any(|a| a.to_ascii_lowercase().contains(*tok))
            || cmd.keywords.iter().any(|k| k.to_ascii_lowercase().contains(*tok))
    }) {
        return None;
    }

    let mut score: i64 = 0;

    if canonical_lower == normalized {
        score += 10000;
    } else if canonical_lower.starts_with(normalized) {
        score += 5000;
    } else if title_lower.starts_with(normalized) {
        score += 4000;
    } else if tokens.iter().all(|t| canonical_lower.contains(t)) {
        score += 3000;
    } else if tokens.iter().all(|t| {
        cmd.aliases.iter().any(|a| a.to_ascii_lowercase().contains(t))
    }) {
        score += 2000;
    } else if tokens.iter().all(|t| title_lower.contains(t)) {
        score += 1500;
    } else if tokens.iter().all(|t| desc_lower.contains(t)) {
        score += 1000;
    } else if tokens.iter().all(|t| {
        cmd.keywords.iter().any(|k| k.to_ascii_lowercase().contains(t))
    }) {
        score += 800;
    } else if tokens.iter().all(|t| group_lower.contains(t)) {
        score += 500;
    }

    Some(CommandMatch { command: cmd.clone(), score })
}

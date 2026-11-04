use chrono::{DateTime, Utc};

pub fn time_ago(dt: DateTime<Utc>) -> String {
    let now = Utc::now();
    let diff = now.signed_duration_since(dt);
    if diff.num_seconds() < 60 {
        format!("{}s ago", diff.num_seconds())
    } else if diff.num_minutes() < 60 {
        format!("{}m ago", diff.num_minutes())
    } else if diff.num_hours() < 24 {
        format!("{}h ago", diff.num_hours())
    } else if diff.num_days() < 30 {
        format!("{}d ago", diff.num_days())
    } else {
        format!("{}mo ago", diff.num_days() / 30)
    }
}

/// Truncate text to `max_chars` characters, appending an ellipsis if truncated.
/// Uses character count (not byte count) so multi-byte chars are preserved.
pub fn truncate_for_display(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    if max_chars <= 1 {
        return "…".to_string();
    }
    let truncated: String = text.chars().take(max_chars - 1).collect();
    format!("{}…", truncated)
}

#[cfg(test)]
mod tests {
    use super::truncate_for_display;

    #[test]
    fn truncate_short_text_unchanged() {
        assert_eq!(truncate_for_display("hello", 10), "hello");
    }

    #[test]
    fn truncate_exact_length_unchanged() {
        assert_eq!(truncate_for_display("hello", 5), "hello");
    }

    #[test]
    fn truncate_long_text_gets_ellipsis() {
        assert_eq!(truncate_for_display("hello world", 8), "hello w…");
    }

    #[test]
    fn truncate_very_small_max() {
        assert_eq!(truncate_for_display("hello", 1), "…");
    }

    #[test]
    fn truncate_multibyte_preserved() {
        assert_eq!(truncate_for_display("héllo wörld", 8), "héllo w…");
    }
}

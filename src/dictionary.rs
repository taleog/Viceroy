use anyhow::Result;
use std::process::Command;

// Lightweight check if query is a define command
pub fn is_define_command(query: &str) -> Option<String> {
    let lower = query.to_lowercase();

    if lower.starts_with("define ") {
        return Some(query[7..].trim().to_string());
    }

    if lower.starts_with("def ") {
        return Some(query[4..].trim().to_string());
    }

    if lower.starts_with("d ") && query.len() > 2 {
        return Some(query[2..].trim().to_string());
    }

    None
}

pub fn open_dictionary(word: &str) -> Result<()> {
    Command::new("open")
        .arg(format!("dict://{}", word))
        .spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test is_define_command with "define" prefix
    #[test]
    fn test_is_define_command_define_prefix() {
        let result = is_define_command("define hello");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "hello");
    }

    #[test]
    fn test_is_define_command_define_with_spaces() {
        let result = is_define_command("define   world");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "world");
    }

    #[test]
    fn test_is_define_command_define_phrase() {
        let result = is_define_command("define machine learning");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "machine learning");
    }

    // Test is_define_command with "def" prefix
    #[test]
    fn test_is_define_command_def_prefix() {
        let result = is_define_command("def hello");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "hello");
    }

    #[test]
    fn test_is_define_command_def_with_spaces() {
        let result = is_define_command("def   world");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "world");
    }

    // Test is_define_command with "d" prefix
    #[test]
    fn test_is_define_command_d_prefix() {
        let result = is_define_command("d hello");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "hello");
    }

    #[test]
    fn test_is_define_command_d_with_spaces() {
        let result = is_define_command("d   world");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "world");
    }

    // Test case insensitivity
    #[test]
    fn test_is_define_command_case_insensitive_define() {
        assert!(is_define_command("DEFINE test").is_some());
        assert!(is_define_command("Define test").is_some());
        assert!(is_define_command("DeFiNe test").is_some());
    }

    #[test]
    fn test_is_define_command_case_insensitive_def() {
        assert!(is_define_command("DEF test").is_some());
        assert!(is_define_command("Def test").is_some());
        assert!(is_define_command("DeF test").is_some());
    }

    #[test]
    fn test_is_define_command_case_insensitive_d() {
        assert!(is_define_command("D test").is_some());
    }

    // Test invalid inputs
    #[test]
    fn test_is_define_command_no_match() {
        assert!(is_define_command("hello world").is_none());
        assert!(is_define_command("definition").is_none());
        assert!(is_define_command("defend").is_none());
    }

    #[test]
    fn test_is_define_command_empty_query() {
        assert!(is_define_command("define ").is_some()); // Returns empty string
        assert!(is_define_command("def ").is_some());
    }

    #[test]
    fn test_is_define_command_just_prefix() {
        // "d " needs more than 2 characters total
        let result = is_define_command("d ");
        // It should still work since len > 2
        assert!(result.is_some());
    }

    #[test]
    fn test_is_define_command_d_too_short() {
        // "d" alone should not match (needs space and content)
        assert!(is_define_command("d").is_none());
    }

    // Test edge cases
    #[test]
    fn test_is_define_command_with_numbers() {
        let result = is_define_command("define 42");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "42");
    }

    #[test]
    fn test_is_define_command_with_special_chars() {
        let result = is_define_command("define café");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "café");
    }

    #[test]
    fn test_is_define_command_trims_word() {
        let result = is_define_command("define   hello  ");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "hello");
    }

    // Test that similar words don't trigger false positives
    #[test]
    fn test_is_define_command_no_false_positives() {
        assert!(is_define_command("definitely not").is_none());
        assert!(is_define_command("defiant").is_none());
        assert!(is_define_command("defender").is_none());
        assert!(is_define_command("defile").is_none());
    }
}

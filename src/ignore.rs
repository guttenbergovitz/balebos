use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Loads ignore patterns from the given file path
/// Returns a Vec of patterns, or an empty Vec if the file doesn't exist
pub fn load_ignore_patterns(filepath: &str) -> Vec<String> {
    let path = Path::new(filepath);

    // If the file doesn't exist, return an empty Vec (don't treat as error)
    if !path.exists() {
        return Vec::new();
    }

    // Attempt to open the file
    match File::open(path) {
        Ok(file) => {
            let reader = BufReader::new(file);
            let mut patterns = Vec::new();

            for line in reader.lines().map_while(Result::ok) {
                let trimmed = line.trim();
                if !trimmed.is_empty() && !trimmed.starts_with('#') {
                    patterns.push(trimmed.to_string());
                }
            }

            patterns
        }
        Err(_) => Vec::new(),
    }
}

/// Checks if a name matches any of the ignore patterns
/// Returns true if the name should be ignored
pub fn is_ignored(name: &str, patterns: &[String]) -> bool {
    for pattern in patterns {
        // Exact match
        if pattern == name {
            return true;
        }

        // Simple glob match (only supporting * wildcard for now)
        if pattern.contains('*') {
            // Convert glob pattern to regex
            // Replace * with .* (any characters)
            let regex_pattern = pattern.replace('.', "\\.").replace('*', ".*");

            // Try to create and use the regex
            if let Ok(regex) = regex::Regex::new(&format!("^{}$", regex_pattern)) {
                if regex.is_match(name) {
                    return true;
                }
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let patterns = vec!["example.com".to_string(), "test.org".to_string()];

        assert!(is_ignored("example.com", &patterns));
        assert!(is_ignored("test.org", &patterns));
        assert!(!is_ignored("other.com", &patterns));
    }

    #[test]
    fn test_glob_match() {
        let patterns = vec!["*.test.*".to_string(), "dev-*".to_string()];

        assert!(is_ignored("foo.test.com", &patterns));
        assert!(is_ignored("bar.test.org", &patterns));
        assert!(is_ignored("dev-server", &patterns));
        assert!(!is_ignored("production.com", &patterns));
        assert!(!is_ignored("test-dev.com", &patterns)); // Doesn't match *.test.*
    }

    #[test]
    fn test_invalid_pattern() {
        let patterns = vec!["[invalid".to_string()]; // Invalid regex

        // Should not crash and should return false
        assert!(!is_ignored("example.com", &patterns));
    }
}

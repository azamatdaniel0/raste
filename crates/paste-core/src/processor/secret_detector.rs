use std::sync::OnceLock;

use regex::Regex;

/// Detects secrets, API tokens, private keys, and high-entropy credential strings.
pub fn is_secret_or_confidential(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.len() > 8192 {
        return false;
    }

    static SECRET_PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    let patterns = SECRET_PATTERNS.get_or_init(|| {
        [
            // PEM private keys
            r"-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----",
            // OpenAI / Anthropic / Stripe style sk- keys
            r"\bsk-(?:proj-|ant-|live-|test-)?[A-Za-z0-9_\-]{20,}\b",
            // GitHub PATs
            r"\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{20,}\b",
            r"\bgithub_pat_[A-Za-z0-9_]{30,}\b",
            // AWS Access Key ID
            r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b",
            // Slack tokens
            r"\bxox[baprs]-[A-Za-z0-9\-]{10,}\b",
            // Google API keys
            r"\bAIza[0-9A-Za-z\-_]{35}\b",
            // JWT tokens
            r"\beyJ[A-Za-z0-9_\-]{10,}\.eyJ[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}\b",
        ]
        .iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect()
    });

    if patterns.iter().any(|re| re.is_match(trimmed)) {
        return true;
    }

    // Heuristic for single-line high-entropy tokens/passwords (no whitespace, 16..128 chars, mixed classes)
    if !trimmed.contains(char::is_whitespace)
        && !trimmed.starts_with("http://")
        && !trimmed.starts_with("https://")
        && !trimmed.contains('/')
        && !trimmed.contains('\\')
        && (16..=128).contains(&trimmed.len())
    {
        let has_upper = trimmed.chars().any(|c| c.is_ascii_uppercase());
        let has_lower = trimmed.chars().any(|c| c.is_ascii_lowercase());
        let has_digit = trimmed.chars().any(|c| c.is_ascii_digit());
        let has_symbol = trimmed
            .chars()
            .any(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_');
        let classes = [has_upper, has_lower, has_digit, has_symbol]
            .into_iter()
            .filter(|&b| b)
            .count();

        if classes >= 3 && shannon_entropy(trimmed) >= 3.6 {
            return true;
        }
    }

    false
}

fn shannon_entropy(s: &str) -> f64 {
    let mut counts = [0usize; 256];
    let bytes = s.as_bytes();
    for &b in bytes {
        counts[b as usize] += 1;
    }
    let len = bytes.len() as f64;
    counts
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / len;
            -p * p.log2()
        })
        .sum()
}

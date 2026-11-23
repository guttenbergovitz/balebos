const TWO_PART_PUBLIC_SUFFIXES: &[&str] = &[
    "co.uk", "org.uk", "gov.uk", "ac.uk", "net.uk", "me.uk", "ltd.uk", "plc.uk", "com.au",
    "org.au", "net.au", "edu.au", "gov.au", "co.jp", "ne.jp", "or.jp", "ac.jp", "co.kr", "co.nz",
    "co.za", "co.in", "co.il", "com.br", "com.cn", "com.mx", "com.pl", "org.pl", "net.pl",
    "gov.pl", "edu.pl", "waw.pl", "com.tr", "com.ua", "com.sg", "com.hk",
];

/// Extract the registrable domain (eTLD+1) using a public-suffix table
/// rather than naive label counting.
pub fn registrable_domain(domain: &str) -> String {
    let norm = normalize_domain(domain);
    let host = norm
        .split('/')
        .next()
        .unwrap_or(&norm)
        .trim_end_matches('.');
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() < 2 {
        return host.to_string();
    }
    let two_part = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
    let needed = if TWO_PART_PUBLIC_SUFFIXES.contains(&two_part.as_str()) {
        3
    } else {
        2
    };
    if parts.len() <= needed {
        host.to_string()
    } else {
        parts[parts.len() - needed..].join(".")
    }
}

/// Validates that a string looks like a plausible domain name.
///
/// This is a lightweight structural check (no DNS lookup). It verifies:
/// - Non-empty after normalization
/// - Contains at least one dot
/// - Labels are non-empty and alphanumeric (plus hyphens)
/// - No label starts or ends with a hyphen
/// - TLD is at least 2 characters
pub fn validate_domain(domain: &str) -> Result<(), String> {
    let normalized = normalize_domain(domain);

    if normalized.is_empty() {
        return Err("domain is empty".to_string());
    }

    // Strip any remaining path for validation (we only validate the host part)
    let host = normalized.split('/').next().unwrap_or(&normalized);

    let labels: Vec<&str> = host.split('.').collect();

    if labels.len() < 2 {
        return Err(format!("'{}' has no TLD (missing dot)", host));
    }

    for label in &labels {
        if label.is_empty() {
            return Err(format!("'{}' contains empty label", host));
        }

        if label.starts_with('-') || label.ends_with('-') {
            return Err(format!("label '{}' starts or ends with hyphen", label));
        }

        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(format!("label '{}' contains invalid characters", label));
        }
    }

    let tld = labels.last().unwrap();
    if tld.len() < 2 {
        return Err(format!("TLD '{}' is too short", tld));
    }

    Ok(())
}

/// Normalizes a domain name to a canonical form.
///
/// Transformations applied (in order):
/// 1. Lowercase
/// 2. Strip protocol prefix (`http://`, `https://`, etc.)
/// 3. Strip `www.` prefix
/// 4. Strip trailing `/`
pub fn normalize_domain(domain: &str) -> String {
    let mut result = domain.to_lowercase();

    if let Some(pos) = result.find("://") {
        result = result[pos + 3..].to_string();
    }

    if let Some(stripped) = result.strip_prefix("www.") {
        result = stripped.to_string();
    }

    if let Some(stripped) = result.strip_suffix('/') {
        result = stripped.to_string();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_domain() {
        assert_eq!(normalize_domain("example.com"), "example.com");
    }

    #[test]
    fn test_uppercase() {
        assert_eq!(normalize_domain("EXAMPLE.COM"), "example.com");
        assert_eq!(normalize_domain("Example.Com"), "example.com");
    }

    #[test]
    fn test_strip_https() {
        assert_eq!(normalize_domain("https://example.com"), "example.com");
    }

    #[test]
    fn test_strip_http() {
        assert_eq!(normalize_domain("http://example.com"), "example.com");
    }

    #[test]
    fn test_strip_www() {
        assert_eq!(normalize_domain("www.example.com"), "example.com");
    }

    #[test]
    fn test_strip_trailing_slash() {
        assert_eq!(normalize_domain("example.com/"), "example.com");
    }

    #[test]
    fn test_combined() {
        assert_eq!(normalize_domain("HTTPS://WWW.Example.Com/"), "example.com");
    }

    #[test]
    fn test_subdomain_preserved() {
        assert_eq!(normalize_domain("api.example.com"), "api.example.com");
    }

    #[test]
    fn test_path_not_stripped() {
        assert_eq!(
            normalize_domain("https://example.com/path"),
            "example.com/path"
        );
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(normalize_domain(""), "");
    }

    // --- validate_domain tests ---

    #[test]
    fn test_valid_domain() {
        assert!(validate_domain("example.com").is_ok());
        assert!(validate_domain("sub.example.com").is_ok());
        assert!(validate_domain("my-site.co.uk").is_ok());
    }

    #[test]
    fn test_valid_domain_with_protocol() {
        assert!(validate_domain("https://example.com").is_ok());
        assert!(validate_domain("http://www.example.com").is_ok());
    }

    #[test]
    fn test_empty_domain_rejected() {
        assert!(validate_domain("").is_err());
    }

    #[test]
    fn test_no_tld_rejected() {
        assert!(validate_domain("localhost").is_err());
    }

    #[test]
    fn test_short_tld_rejected() {
        assert!(validate_domain("example.x").is_err());
    }

    #[test]
    fn test_leading_hyphen_rejected() {
        assert!(validate_domain("-example.com").is_err());
    }

    #[test]
    fn test_trailing_hyphen_rejected() {
        assert!(validate_domain("example-.com").is_err());
    }

    #[test]
    fn test_empty_label_rejected() {
        assert!(validate_domain("example..com").is_err());
    }

    #[test]
    fn test_invalid_chars_rejected() {
        assert!(validate_domain("exam ple.com").is_err());
        assert!(validate_domain("exam_ple.com").is_err());
    }
}

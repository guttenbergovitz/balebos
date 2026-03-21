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
        assert_eq!(
            normalize_domain("HTTPS://WWW.Example.Com/"),
            "example.com"
        );
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
}

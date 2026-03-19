// Function to normalize domain names
pub fn normalize_domain(domain: &str) -> String {
    let mut result = domain.to_lowercase();

    // Remove protocol prefix (http://, https://)
    if let Some(pos) = result.find("://") {
        result = result[pos + 3..].to_string();
    }

    // Remove www. prefix
    if result.starts_with("www.") {
        result = result[4..].to_string();
    }

    // Remove trailing slash if present
    if result.ends_with('/') {
        result = result[..result.len() - 1].to_string();
    }

    result
}

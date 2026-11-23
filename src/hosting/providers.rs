//! Provider knowledge base for hosting detection.

use serde::Deserialize;
use std::collections::HashMap;

/// Provider category classification.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCategory {
    Hyperscaler,
    Cdn,
    European,
    Germany,
    Poland,
    Uk,
    UsHosting,
    Edge,
    Other,
}

impl std::fmt::Display for ProviderCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderCategory::Hyperscaler => write!(f, "Hyperscaler"),
            ProviderCategory::Cdn => write!(f, "CDN"),
            ProviderCategory::European => write!(f, "European Cloud"),
            ProviderCategory::Germany => write!(f, "Germany"),
            ProviderCategory::Poland => write!(f, "Poland"),
            ProviderCategory::Uk => write!(f, "UK"),
            ProviderCategory::UsHosting => write!(f, "US Hosting"),
            ProviderCategory::Edge => write!(f, "Edge/PaaS"),
            ProviderCategory::Other => write!(f, "Other"),
        }
    }
}

/// A hosting provider definition with detection patterns.
#[derive(Debug, Clone, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub category: ProviderCategory,
    #[allow(dead_code)]
    pub country: String,
    #[serde(default)]
    pub asn_patterns: Vec<u32>,
    #[serde(default)]
    pub cname_patterns: Vec<String>,
    #[serde(default)]
    pub header_patterns: Vec<String>,
    #[serde(default)]
    pub cdn_name_patterns: Vec<String>,
}

impl Provider {
    /// Check if a CDN name matches this provider.
    pub fn matches_cdn_name(&self, cdn_name: &str) -> bool {
        let cdn_lower = cdn_name.to_lowercase();
        self.cdn_name_patterns
            .iter()
            .any(|p| cdn_lower.contains(&p.to_lowercase()))
    }

    /// Check if a CNAME matches a pattern for this provider on label boundaries.
    pub fn matches_cname(&self, cname: &str) -> bool {
        let cname_norm = cname.trim().trim_end_matches('.').to_lowercase();
        if cname_norm.is_empty() {
            return false;
        }
        self.cname_patterns.iter().any(|raw| {
            let pat = raw.trim().trim_matches('.').to_lowercase();
            if pat.is_empty() {
                return false;
            }
            if pat.ends_with('-') {
                // Label-prefix pattern such as `awsdns-`
                cname_norm.starts_with(&pat) || cname_norm.contains(&format!(".{pat}"))
            } else {
                cname_norm == pat || cname_norm.ends_with(&format!(".{pat}"))
            }
        })
    }

    /// Check if an ASN matches this provider.
    #[allow(dead_code)]
    pub fn matches_asn(&self, asn: u32) -> bool {
        self.asn_patterns.contains(&asn)
    }

    /// Check if a header value matches this provider.
    pub fn matches_header(&self, header: &str) -> bool {
        let header_lower = header.to_lowercase();
        self.header_patterns
            .iter()
            .any(|p| header_lower.contains(&p.to_lowercase()))
    }

    /// Check if this provider is a CDN.
    pub fn is_cdn(&self) -> bool {
        self.category == ProviderCategory::Cdn
    }
}

/// Raw JSON structure for the providers file.
#[derive(Deserialize)]
struct ProvidersFile {
    providers: Vec<Provider>,
    #[allow(dead_code)]
    categories: HashMap<String, String>,
}

/// The provider knowledge base loaded from JSON.
#[derive(Debug, Clone)]
pub struct ProviderKnowledgeBase {
    providers: Vec<Provider>,
    asn_index: HashMap<u32, Vec<usize>>,
}

impl ProviderKnowledgeBase {
    /// Load the provider knowledge base from embedded JSON.
    pub fn load() -> Self {
        let json = include_str!("providers.json");
        let file: ProvidersFile =
            serde_json::from_str(json).expect("Failed to parse providers.json");

        // Build ASN index for fast lookups
        let mut asn_index: HashMap<u32, Vec<usize>> = HashMap::new();
        for (idx, provider) in file.providers.iter().enumerate() {
            for &asn in &provider.asn_patterns {
                asn_index.entry(asn).or_default().push(idx);
            }
        }

        Self {
            providers: file.providers,
            asn_index,
        }
    }

    /// Get all providers.
    #[allow(dead_code)]
    pub fn providers(&self) -> &[Provider] {
        &self.providers
    }

    /// Find providers by CDN name (from httpx cdn_name field).
    pub fn find_by_cdn_name(&self, cdn_name: &str) -> Vec<&Provider> {
        self.providers
            .iter()
            .filter(|p| p.matches_cdn_name(cdn_name))
            .collect()
    }

    /// Find providers by CNAME pattern.
    pub fn find_by_cname(&self, cname: &str) -> Vec<&Provider> {
        self.providers
            .iter()
            .filter(|p| p.matches_cname(cname))
            .collect()
    }

    /// Find providers by ASN.
    pub fn find_by_asn(&self, asn: u32) -> Vec<&Provider> {
        self.asn_index
            .get(&asn)
            .map(|indices| indices.iter().map(|&i| &self.providers[i]).collect())
            .unwrap_or_default()
    }

    /// Find providers by header value.
    pub fn find_by_header(&self, header: &str) -> Vec<&Provider> {
        self.providers
            .iter()
            .filter(|p| p.matches_header(header))
            .collect()
    }

    /// Get a provider by ID.
    #[allow(dead_code)]
    pub fn get(&self, id: &str) -> Option<&Provider> {
        self.providers.iter().find(|p| p.id == id)
    }
}

impl Default for ProviderKnowledgeBase {
    fn default() -> Self {
        Self::load()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_providers() {
        let kb = ProviderKnowledgeBase::load();
        assert!(!kb.providers().is_empty());
    }

    #[test]
    fn test_find_by_cdn_name() {
        let kb = ProviderKnowledgeBase::load();
        let matches = kb.find_by_cdn_name("cloudflare");
        assert!(!matches.is_empty());
        assert!(matches.iter().any(|p| p.id == "cloudflare"));
    }

    #[test]
    fn test_find_by_cname() {
        let kb = ProviderKnowledgeBase::load();
        let matches = kb.find_by_cname("example.amazonaws.com");
        assert!(!matches.is_empty());
        assert!(matches.iter().any(|p| p.id == "aws"));
    }

    #[test]
    fn test_cname_respects_label_boundaries() {
        let kb = ProviderKnowledgeBase::load();
        assert!(kb.find_by_cname("notgoogle.com").is_empty());
        assert!(kb.find_by_cname("myamazonaws.com").is_empty());
        assert!(kb
            .find_by_cname("ghs.google.com.")
            .iter()
            .any(|p| p.id == "gcp"));
    }

    #[test]
    fn test_kb_integrity_no_duplicate_ids_or_generic_headers() {
        let kb = ProviderKnowledgeBase::load();
        let mut seen = std::collections::HashSet::new();
        for p in kb.providers() {
            assert!(seen.insert(&p.id), "duplicate provider id: {}", p.id);
            for h in &p.header_patterns {
                let lower = h.to_lowercase();
                assert!(
                    lower != "microsoft-iis" && lower != "nginx" && lower != "apache",
                    "generic web server header '{h}' must not be in provider KB ({})",
                    p.id
                );
            }
        }
    }

    #[test]
    fn test_find_by_asn_cloudflare() {
        let kb = ProviderKnowledgeBase::load();
        let matches = kb.find_by_asn(13335);
        assert!(!matches.is_empty());
        assert!(matches.iter().any(|p| p.id == "cloudflare"));
    }

    #[test]
    fn test_provider_is_cdn() {
        let kb = ProviderKnowledgeBase::load();
        let cloudflare = kb.get("cloudflare").unwrap();
        assert!(cloudflare.is_cdn());

        let aws = kb.get("aws").unwrap();
        assert!(!aws.is_cdn());
    }
}

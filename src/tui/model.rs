use crate::app::probe_dns::DnsObservation;
use crate::app::probe_http::HttpObservation;
use crate::storage::{read_records, JsonlStorage};
use std::collections::{HashMap, BTreeMap};
use std::io;

/// Aggregated state of a single asset for the TUI
#[derive(Debug, Clone)]
pub struct AssetState {
    pub name: String,
    pub latest_dns: Option<DnsObservation>,
    pub latest_http: Option<HttpObservation>,
}

impl AssetState {
    /// Detects if the asset has an external redirect (to a different root domain).
    /// Returns Some(target_domain) if it's external, None otherwise.
    ///
    /// Same-domain redirects are NOT considered external:
    /// - non-www to www (or vice versa)
    /// - to a subfolder (e.g. example.com -> example.com/en)
    /// - between subdomains of the same root domain (e.g. shop.josera.de -> template.josera.de)
    pub fn external_redirect(&self) -> Option<String> {
        let http = self.latest_http.as_ref()?;

        // 1. Clean up the current asset name (the intended target)
        let current_domain = self.name.to_lowercase()
            .replace("www.", "")
            .trim_matches('.')
            .to_string();

        // 2. Determine the true final destination.
        //    Prefer final_url, but also check redirect_chain as fallback
        //    (the last 3xx hop's redirect_to is the actual destination).
        let destination_url = http.final_url.clone().or_else(|| {
            http.redirect_chain.iter()
                .rev()
                .find(|hop| (300..400).contains(&(hop.status as u32)))
                .map(|hop| hop.redirect_to.clone())
        })?;

        // 3. Extract and clean the domain from destination URL (strip protocol, path, port, www)
        let final_domain = extract_domain_from_url(&destination_url);

        if final_domain.is_empty() || final_domain == current_domain {
            return None;
        }

        // 4. Check if both domains share the same root domain.
        //    A redirect between subdomains of the same root is NOT external.
        if is_same_root_domain(&current_domain, &final_domain) {
            return None;
        }

        Some(final_domain)
    }
}

/// Extracts a clean domain from a URL, stripping protocol, path, port, and www prefix.
fn extract_domain_from_url(url: &str) -> String {
    let lower = url.to_lowercase();
    let without_proto = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .unwrap_or(&lower);
    let host = without_proto.split('/').next().unwrap_or("");
    let host = host.split(':').next().unwrap_or(host);
    host.replace("www.", "")
        .trim_matches('.')
        .to_string()
}

/// Checks whether two domain names share the same root domain.
///
/// Handles common two-part TLDs (e.g. co.uk, com.au) as well as standard TLDs.
/// For example:
/// - `shop.josera.de` and `template.josera.de` → true (root: josera.de)
/// - `josera.de` and `maxcluster.net` → false
/// - `foo.co.uk` and `bar.co.uk` → true (root: co.uk level, need 3 labels)
fn is_same_root_domain(a: &str, b: &str) -> bool {
    let a_parts: Vec<&str> = a.split('.').collect();
    let b_parts: Vec<&str> = b.split('.').collect();

    // Known two-part TLDs where the root domain requires 3 labels
    const TWO_PART_TLDS: &[&str] = &[
        "co.uk", "co.jp", "co.kr", "co.nz", "co.za", "co.in", "co.il",
        "com.au", "com.br", "com.cn", "com.mx", "com.tr", "com.ua",
        "org.uk", "org.au", "net.au", "net.uk",
        "ac.uk", "gov.uk", "edu.au",
    ];

    let needed = if a_parts.len() >= 3 || b_parts.len() >= 3 {
        let a_suffix = if a_parts.len() >= 2 {
            format!("{}.{}", a_parts[a_parts.len()-2], a_parts[a_parts.len()-1])
        } else { String::new() };
        let b_suffix = if b_parts.len() >= 2 {
            format!("{}.{}", b_parts[b_parts.len()-2], b_parts[b_parts.len()-1])
        } else { String::new() };

        if TWO_PART_TLDS.contains(&a_suffix.as_str()) || TWO_PART_TLDS.contains(&b_suffix.as_str()) {
            3
        } else {
            2
        }
    } else {
        2
    };

    if a_parts.len() < needed || b_parts.len() < needed {
        return false;
    }

    let root_a = &a_parts[a_parts.len()-needed..];
    let root_b = &b_parts[b_parts.len()-needed..];
    root_a == root_b
}

/// A group of subdomains under a root domain
#[derive(Debug, Clone)]
pub struct DomainGroup {
    pub root: AssetState,
    pub subdomains: Vec<AssetState>,
}

pub struct TuiModel {
    pub groups: Vec<DomainGroup>,
    pub stats: GlobalStats,
}

pub struct GlobalStats {
    pub total: usize,
    pub alive: usize,
    pub dns_issues: usize,
    pub http_errors: usize,
}

pub fn load_tui_model(storage: &JsonlStorage) -> Result<TuiModel, io::Error> {
    let records = read_records(storage)?;
    
    let mut assets_map: HashMap<String, AssetState> = HashMap::new();

    // 1. Collect all assets
    for item in records.items {
        assets_map.entry(item.name.clone()).or_insert(AssetState {
            name: item.name,
            latest_dns: None,
            latest_http: None,
        });
    }

    for dns in records.dns_observations {
        let entry = assets_map.entry(dns.target.clone()).or_insert(AssetState {
            name: dns.target.clone(),
            latest_dns: None,
            latest_http: None,
        });
        match &entry.latest_dns {
            Some(existing) if dns.checked_at > existing.checked_at => entry.latest_dns = Some(dns),
            None => entry.latest_dns = Some(dns),
            _ => {}
        }
    }

    for http in records.http_observations {
        let entry = assets_map.entry(http.target.clone()).or_insert(AssetState {
            name: http.target.clone(),
            latest_dns: None,
            latest_http: None,
        });
        match &entry.latest_http {
            Some(existing) if http.checked_at > existing.checked_at => entry.latest_http = Some(http),
            None => entry.latest_http = Some(http),
            _ => {}
        }
    }

    // 2. Group subdomains under root domains
    let mut groups: BTreeMap<String, DomainGroup> = BTreeMap::new();
    
    let mut all_assets: Vec<AssetState> = assets_map.into_values().collect();
    all_assets.sort_by(|a, b| a.name.len().cmp(&b.name.len()));

    for asset in all_assets {
        let mut found_parent = false;
        for root_name in groups.keys().cloned().collect::<Vec<_>>() {
            if asset.name.ends_with(&format!(".{}", root_name)) {
                groups.get_mut(&root_name).unwrap().subdomains.push(asset.clone());
                found_parent = true;
                break;
            }
        }

        if !found_parent {
            groups.insert(asset.name.clone(), DomainGroup {
                root: asset,
                subdomains: Vec::new(),
            });
        }
    }

    let mut final_groups: Vec<DomainGroup> = groups.into_values().collect();
    for group in &mut final_groups {
        group.subdomains.sort_by(|a, b| a.name.cmp(&b.name));
    }
    final_groups.sort_by(|a, b| a.root.name.cmp(&b.root.name));

    // 3. Calculate stats
    let mut stats = GlobalStats {
        total: 0,
        alive: 0,
        dns_issues: 0,
        http_errors: 0,
    };

    for group in &final_groups {
        update_stats(&mut stats, &group.root);
        for sub in &group.subdomains {
            update_stats(&mut stats, &sub);
        }
    }

    Ok(TuiModel { groups: final_groups, stats })
}

fn update_stats(stats: &mut GlobalStats, asset: &AssetState) {
    stats.total += 1;
    if let Some(http) = &asset.latest_http {
        if http.status == Some(200) {
            stats.alive += 1;
        } else if http.error_kind.is_some() {
            stats.http_errors += 1;
        }
    }
    if let Some(dns) = &asset.latest_dns {
        if dns.a.is_empty() && dns.aaaa.is_empty() && dns.cname.is_empty() {
            stats.dns_issues += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_root_subdomains() {
        assert!(is_same_root_domain("shop.josera.de", "template.josera.de"));
        assert!(is_same_root_domain("josera.de", "shop.josera.de"));
        assert!(is_same_root_domain("a.b.josera.de", "josera.de"));
    }

    #[test]
    fn test_different_root_domains() {
        assert!(!is_same_root_domain("josera.de", "maxcluster.net"));
        assert!(!is_same_root_domain("phonebook.josera.de", "convertexperiments.com"));
    }

    #[test]
    fn test_same_domain_exact() {
        assert!(is_same_root_domain("josera.de", "josera.de"));
    }

    #[test]
    fn test_two_part_tld() {
        assert!(is_same_root_domain("foo.example.co.uk", "bar.example.co.uk"));
        assert!(!is_same_root_domain("foo.example.co.uk", "foo.other.co.uk"));
        // bare co.uk shouldn't match a real domain under co.uk
        assert!(!is_same_root_domain("co.uk", "example.co.uk"));
    }

    #[test]
    fn test_external_redirect_same_root() {
        let asset = AssetState {
            name: "shop.josera.de".to_string(),
            latest_dns: None,
            latest_http: Some(HttpObservation {
                item_type: "http_check".to_string(),
                id: "test".to_string(),
                asset_type: "domain".to_string(),
                target: "shop.josera.de".to_string(),
                status: Some(200),
                response_time_ms: 100,
                final_url: Some("https://template.josera.de/path".to_string()),
                redirect_chain: vec![],
                tls: None,
                tech: vec![],
                webserver: None,
                title: None,
                cdn_name: None,
                favicon_hash: None,
                checked_at: chrono::Utc::now(),
                source: "httpx".to_string(),
                error_kind: None,
                error_message: None,
            }),
        };
        // Same root domain → NOT external
        assert!(asset.external_redirect().is_none());
    }

    #[test]
    fn test_external_redirect_different_root() {
        let asset = AssetState {
            name: "vwo.josera.de".to_string(),
            latest_dns: None,
            latest_http: Some(HttpObservation {
                item_type: "http_check".to_string(),
                id: "test".to_string(),
                asset_type: "domain".to_string(),
                target: "vwo.josera.de".to_string(),
                status: Some(200),
                response_time_ms: 100,
                final_url: Some("https://mailhog.c-1694.maxcluster.net".to_string()),
                redirect_chain: vec![],
                tls: None,
                tech: vec![],
                webserver: None,
                title: None,
                cdn_name: None,
                favicon_hash: None,
                checked_at: chrono::Utc::now(),
                source: "httpx".to_string(),
                error_kind: None,
                error_message: None,
            }),
        };
        // Different root domain → IS external
        assert_eq!(asset.external_redirect(), Some("mailhog.c-1694.maxcluster.net".to_string()));
    }

    #[test]
    fn test_external_redirect_www_not_external() {
        let asset = AssetState {
            name: "example.com".to_string(),
            latest_dns: None,
            latest_http: Some(HttpObservation {
                item_type: "http_check".to_string(),
                id: "test".to_string(),
                asset_type: "domain".to_string(),
                target: "example.com".to_string(),
                status: Some(200),
                response_time_ms: 100,
                final_url: Some("https://www.example.com/en".to_string()),
                redirect_chain: vec![],
                tls: None,
                tech: vec![],
                webserver: None,
                title: None,
                cdn_name: None,
                favicon_hash: None,
                checked_at: chrono::Utc::now(),
                source: "httpx".to_string(),
                error_kind: None,
                error_message: None,
            }),
        };
        // www + subfolder → NOT external
        assert!(asset.external_redirect().is_none());
    }

    #[test]
    fn test_external_redirect_via_chain_fallback() {
        use crate::app::probe_http::RedirectHop;

        // Simulates josera-estonia.com: final_url is wrongly set to self,
        // but redirect_chain shows a 301 to josera.com/et
        let asset = AssetState {
            name: "josera-estonia.com".to_string(),
            latest_dns: None,
            latest_http: Some(HttpObservation {
                item_type: "http_check".to_string(),
                id: "test".to_string(),
                asset_type: "domain".to_string(),
                target: "josera-estonia.com".to_string(),
                status: Some(200),
                response_time_ms: 100,
                final_url: Some("https://josera.com/et".to_string()),
                redirect_chain: vec![
                    RedirectHop {
                        url: "https://josera-estonia.com/".to_string(),
                        status: 301,
                        redirect_to: "https://josera.com/et".to_string(),
                    },
                ],
                tls: None,
                tech: vec![],
                webserver: None,
                title: None,
                cdn_name: None,
                favicon_hash: None,
                checked_at: chrono::Utc::now(),
                source: "httpx".to_string(),
                error_kind: None,
                error_message: None,
            }),
        };
        // Different root domain → IS external
        assert_eq!(asset.external_redirect(), Some("josera.com".to_string()));
    }

    #[test]
    fn test_external_redirect_chain_only_no_final_url() {
        use crate::app::probe_http::RedirectHop;

        // final_url is None, but redirect_chain has the data
        let asset = AssetState {
            name: "josera-estonia.com".to_string(),
            latest_dns: None,
            latest_http: Some(HttpObservation {
                item_type: "http_check".to_string(),
                id: "test".to_string(),
                asset_type: "domain".to_string(),
                target: "josera-estonia.com".to_string(),
                status: Some(200),
                response_time_ms: 100,
                final_url: None,
                redirect_chain: vec![
                    RedirectHop {
                        url: "https://josera-estonia.com/".to_string(),
                        status: 301,
                        redirect_to: "https://josera.com/et".to_string(),
                    },
                ],
                tls: None,
                tech: vec![],
                webserver: None,
                title: None,
                cdn_name: None,
                favicon_hash: None,
                checked_at: chrono::Utc::now(),
                source: "httpx".to_string(),
                error_kind: None,
                error_message: None,
            }),
        };
        assert_eq!(asset.external_redirect(), Some("josera.com".to_string()));
    }
}

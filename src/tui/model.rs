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
    
    // Sort assets by name length to process potential roots (shorter names) first
    let mut all_assets: Vec<AssetState> = assets_map.into_values().collect();
    all_assets.sort_by(|a, b| a.name.len().cmp(&b.name.len()));

    for asset in all_assets {
        let mut found_parent = false;
        // Simple heuristic: check if asset name ends with ".<existing_root>"
        // In a more robust version, we could use TLD libraries or proper domain parsing.
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

use crate::app::probe::{Confidence, DetectionMethod, DnsObservation, HttpObservation};
use crate::org::{
    BrandId, BrandRecord, CompanyId, CompanyRecord, DomainRegistration, PersonId, PersonRecord,
    ProjectId, ProjectRecord, RoleAssignmentRecord,
};
use crate::storage::StorageEngine;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// Hosting information for display in TUI
#[derive(Debug, Clone)]
pub struct HostingInfo {
    pub provider_name: Option<String>,
    pub provider_category: Option<String>,
    pub cdn_detected: bool,
    pub cdn_name: Option<String>,
    pub confidence: Confidence,
    pub detection_method: DetectionMethod,
    pub evidence: Vec<String>,
}

/// Person with their role in a project
#[derive(Debug, Clone)]
pub struct PersonWithRole {
    pub name: String,
    pub email: String,
    pub role: String,
}

/// Company with their role in a project
#[derive(Debug, Clone)]
pub struct CompanyWithRole {
    pub name: String,
    pub role: String,
}

/// External contractor company with their contractor role in a project
#[derive(Debug, Clone)]
pub struct ContractorWithRole {
    pub name: String,
    pub role: String,
}

/// Organizational context for an asset
#[derive(Debug, Clone)]
pub struct ProjectContext {
    pub project_name: String,
    pub brand_name: String,
    pub company_name: String,
    pub assigned_persons: Vec<PersonWithRole>,
    pub assigned_companies: Vec<CompanyWithRole>,
}

/// Aggregated state of a single asset for the TUI
#[derive(Debug, Clone)]
pub struct AssetState {
    pub name: String,
    pub latest_dns: Option<DnsObservation>,
    pub latest_http: Option<HttpObservation>,
    pub registration: Option<DomainRegistration>,
    pub project: Option<ProjectContext>,
    pub hosting: Option<HostingInfo>,
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
        let current_domain = self
            .name
            .to_lowercase()
            .replace("www.", "")
            .trim_matches('.')
            .to_string();

        // 2. Determine the true final destination.
        //    Prefer final_url, but also check redirect_chain as fallback
        //    (the last 3xx hop's redirect_to is the actual destination).
        let destination_url = http.final_url.clone().or_else(|| {
            http.redirect_chain
                .iter()
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
    host.replace("www.", "").trim_matches('.').to_string()
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
        "co.uk", "co.jp", "co.kr", "co.nz", "co.za", "co.in", "co.il", "com.au", "com.br",
        "com.cn", "com.mx", "com.tr", "com.ua", "org.uk", "org.au", "net.au", "net.uk", "ac.uk",
        "gov.uk", "edu.au",
    ];

    let needed = if a_parts.len() >= 3 || b_parts.len() >= 3 {
        let a_suffix = if a_parts.len() >= 2 {
            format!(
                "{}.{}",
                a_parts[a_parts.len() - 2],
                a_parts[a_parts.len() - 1]
            )
        } else {
            String::new()
        };
        let b_suffix = if b_parts.len() >= 2 {
            format!(
                "{}.{}",
                b_parts[b_parts.len() - 2],
                b_parts[b_parts.len() - 1]
            )
        } else {
            String::new()
        };

        if TWO_PART_TLDS.contains(&a_suffix.as_str()) || TWO_PART_TLDS.contains(&b_suffix.as_str())
        {
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

    let root_a = &a_parts[a_parts.len() - needed..];
    let root_b = &b_parts[b_parts.len() - needed..];
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
    pub asset_tags: std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
}

pub struct GlobalStats {
    pub total: usize,
    pub alive: usize,
    pub dns_issues: usize,
    pub http_errors: usize,
}

// ---------------------------------------------------------------------------
// Organization Model
// ---------------------------------------------------------------------------

/// Flattened view of a project with resolved names for display
#[derive(Debug, Clone)]
pub struct ProjectView {
    pub id: ProjectId,
    pub name: String,
    pub brand_name: String,
    pub company_name: String,
    pub asset_count: usize,
    pub person_count: usize,
    pub company_count: usize,
    pub importance: Option<u8>,
    pub contractors: Vec<ContractorWithRole>,
    pub assigned_persons: Vec<PersonWithRole>,
    pub assigned_companies: Vec<CompanyWithRole>,
}

/// Organization data for the Organization and Projects tabs
#[derive(Debug, Clone, Default)]
pub struct OrgModel {
    pub persons: Vec<PersonRecord>,
    pub companies: Vec<CompanyRecord>,
    pub brands: Vec<BrandRecord>,
    pub projects: Vec<ProjectRecord>,
    pub project_views: Vec<ProjectView>,
    pub assets_by_project: HashMap<ProjectId, Vec<String>>,
    // Lookup maps for quick access (reserved for future use)
    #[allow(dead_code)]
    pub persons_by_id: HashMap<PersonId, PersonRecord>,
    #[allow(dead_code)]
    pub companies_by_id: HashMap<CompanyId, CompanyRecord>,
    #[allow(dead_code)]
    pub brands_by_id: HashMap<BrandId, BrandRecord>,
    #[allow(dead_code)]
    pub projects_by_id: HashMap<ProjectId, ProjectRecord>,
}

impl OrgModel {
    pub fn load(storage: &Arc<dyn StorageEngine>) -> crate::error::Result<Self> {
        let records = storage.read_all_records()?;

        let persons_by_id: HashMap<_, _> =
            records.persons.iter().map(|p| (p.id, p.clone())).collect();
        let companies_by_id: HashMap<_, _> = records
            .companies
            .iter()
            .map(|c| (c.id, c.clone()))
            .collect();
        let brands_by_id: HashMap<_, _> =
            records.brands.iter().map(|b| (b.id, b.clone())).collect();
        let projects_by_id: HashMap<_, _> =
            records.projects.iter().map(|p| (p.id, p.clone())).collect();

        // Build project views with resolved names
        let mut project_views = Vec::new();
        let mut assets_by_project: HashMap<ProjectId, Vec<String>> = HashMap::new();

        for asset_assignment in &records.asset_assignments {
            assets_by_project
                .entry(asset_assignment.project)
                .or_default()
                .push(asset_assignment.host.clone());
        }

        for project in &records.projects {
            // Get company and derive brand from company
            let company = companies_by_id.get(&project.company);
            let company_name = company
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "Unknown".to_string());
            let brand_name = company
                .and_then(|c| c.brand)
                .and_then(|brand_id| brands_by_id.get(&brand_id))
                .map(|b| b.name.clone())
                .unwrap_or_else(|| "External".to_string());

            // Count assets assigned to this project
            let asset_count = records
                .asset_assignments
                .iter()
                .filter(|a| a.project == project.id)
                .count();

            // Count persons and companies assigned to this project
            let mut person_count = 0;
            let mut company_count = 0;
            let mut assigned_persons = Vec::new();
            let mut assigned_companies = Vec::new();

            for role in &records.role_assignments {
                if role.project == project.id {
                    if let Some(p_id) = role.person {
                        person_count += 1;
                        if let Some(p) = persons_by_id.get(&p_id) {
                            assigned_persons.push(PersonWithRole {
                                name: format!("{} {}", p.first_name, p.last_name),
                                email: p.email.clone(),
                                role: role.role.to_string(),
                            });
                        }
                    }
                    if let Some(c_id) = role.company {
                        company_count += 1;
                        if let Some(c) = companies_by_id.get(&c_id) {
                            assigned_companies.push(CompanyWithRole {
                                name: c.name.clone(),
                                role: role.role.to_string(),
                            });
                        }
                    }
                }
            }

            let mut contractors = Vec::new();
            for ca in &records.contractor_assignments {
                if ca.project == project.id {
                    if let Some(c) = companies_by_id.get(&ca.company) {
                        contractors.push(ContractorWithRole {
                            name: c.name.clone(),
                            role: ca.role.to_string(),
                        });
                    }
                }
            }

            project_views.push(ProjectView {
                id: project.id,
                name: project.name.clone(),
                brand_name,
                company_name,
                asset_count,
                person_count,
                company_count,
                importance: project.importance,
                contractors,
                assigned_persons,
                assigned_companies,
            });
        }

        Ok(OrgModel {
            persons: records.persons,
            companies: records.companies,
            brands: records.brands,
            projects: records.projects,
            project_views,
            assets_by_project,
            persons_by_id,
            companies_by_id,
            brands_by_id,
            projects_by_id,
        })
    }
}

pub fn load_tui_model(storage: &Arc<dyn StorageEngine>) -> crate::error::Result<TuiModel> {
    let records = storage.read_all_records()?;

    // Build lookup maps for org data
    let persons_map: HashMap<_, _> = records.persons.iter().map(|p| (p.id, p.clone())).collect();
    let companies_map: HashMap<_, _> = records
        .companies
        .iter()
        .map(|c| (c.id, c.clone()))
        .collect();
    let brands_map: HashMap<_, _> = records.brands.iter().map(|b| (b.id, b.clone())).collect();
    let projects_map: HashMap<_, _> = records.projects.iter().map(|p| (p.id, p.clone())).collect();

    // Build asset -> project mapping (most recent assignment wins)
    let mut asset_to_project: HashMap<String, ProjectId> = HashMap::new();
    for assignment in &records.asset_assignments {
        asset_to_project.insert(assignment.host.clone(), assignment.project);
    }

    // Build project -> role assignments mapping
    let mut project_role_assignments: HashMap<ProjectId, Vec<&RoleAssignmentRecord>> =
        HashMap::new();
    for assignment in &records.role_assignments {
        project_role_assignments
            .entry(assignment.project)
            .or_default()
            .push(assignment);
    }

    // Helper to build ProjectContext for an asset
    let build_project_context = |host: &str| -> Option<ProjectContext> {
        let project_id = asset_to_project.get(host)?;
        let project = projects_map.get(project_id)?;
        let company = companies_map.get(&project.company);
        // Derive brand from company
        let brand = company
            .and_then(|c| c.brand)
            .and_then(|brand_id| brands_map.get(&brand_id));

        let role_assignments = project_role_assignments.get(project_id);
        let mut assigned_persons = Vec::new();
        let mut assigned_companies = Vec::new();

        if let Some(assignments) = role_assignments {
            for a in assignments {
                if let Some(person_id) = a.person {
                    if let Some(person) = persons_map.get(&person_id) {
                        assigned_persons.push(PersonWithRole {
                            name: format!("{} {}", person.first_name, person.last_name),
                            email: person.email.clone(),
                            role: a.role.to_string(),
                        });
                    }
                }
                if let Some(company_id) = a.company {
                    if let Some(company) = companies_map.get(&company_id) {
                        assigned_companies.push(CompanyWithRole {
                            name: company.name.clone(),
                            role: a.role.to_string(),
                        });
                    }
                }
            }
        }

        Some(ProjectContext {
            project_name: project.name.clone(),
            brand_name: brand.map(|b| b.name.clone()).unwrap_or_default(),
            company_name: company.map(|c| c.name.clone()).unwrap_or_default(),
            assigned_persons,
            assigned_companies,
        })
    };

    let mut domain_registrations: HashMap<String, DomainRegistration> = HashMap::new();
    for reg in records.domain_registrations {
        domain_registrations.insert(
            reg.domain.clone(),
            DomainRegistration {
                domain: reg.domain,
                registrar: reg.registrar,
                registered_at: reg.registered_at,
                expires_at: reg.expires_at,
                updated_at: reg.updated_at,
                status: reg.status,
                registrant: reg.registrant,
                admin: reg.admin,
                tech: reg.tech,
                source: reg.source,
            },
        );
    }

    // Helper: Get registration for a domain, checking apex if subdomain
    let get_registration = |name: &str| -> Option<DomainRegistration> {
        // First try exact match
        if let Some(reg) = domain_registrations.get(name) {
            return Some(reg.clone());
        }
        // Try apex domain (e.g., www.example.com -> example.com)
        let parts: Vec<&str> = name.split('.').collect();
        if parts.len() > 2 {
            let apex = parts[parts.len() - 2..].join(".");
            if let Some(reg) = domain_registrations.get(&apex) {
                return Some(reg.clone());
            }
        }
        None
    };

    // Build hosting info map (keep latest per target)
    let mut hosting_map: HashMap<String, HostingInfo> = HashMap::new();
    for obs in records.hosting_observations {
        let info = HostingInfo {
            provider_name: obs.provider_name,
            provider_category: obs.provider_category,
            cdn_detected: obs.cdn_detected,
            cdn_name: obs.cdn_name,
            confidence: obs.confidence,
            detection_method: obs.detection_method,
            evidence: obs.evidence,
        };
        hosting_map
            .entry(obs.target.clone())
            .and_modify(|existing| {
                // Keep the latest one (we could compare checked_at but simpler to just replace)
                *existing = info.clone();
            })
            .or_insert(info);
    }

    let mut assets_map: HashMap<String, AssetState> = HashMap::new();

    // 1. Collect all assets
    for item in records.items {
        let project = build_project_context(&item.name);
        let registration = get_registration(&item.name);
        let hosting = hosting_map.get(&item.name).cloned();
        assets_map.entry(item.name.clone()).or_insert(AssetState {
            name: item.name,
            latest_dns: None,
            latest_http: None,
            registration,
            project,
            hosting,
        });
    }

    for dns in records.dns_observations {
        let entry = assets_map.entry(dns.target.clone()).or_insert_with(|| {
            let project = build_project_context(&dns.target);
            let registration = get_registration(&dns.target);
            let hosting = hosting_map.get(&dns.target).cloned();
            AssetState {
                name: dns.target.clone(),
                latest_dns: None,
                latest_http: None,
                registration,
                project,
                hosting,
            }
        });
        match &entry.latest_dns {
            Some(existing) if dns.checked_at > existing.checked_at => entry.latest_dns = Some(dns),
            None => entry.latest_dns = Some(dns),
            _ => {}
        }
    }

    for http in records.http_observations {
        let entry = assets_map.entry(http.target.clone()).or_insert_with(|| {
            let project = build_project_context(&http.target);
            let registration = get_registration(&http.target);
            let hosting = hosting_map.get(&http.target).cloned();
            AssetState {
                name: http.target.clone(),
                latest_dns: None,
                latest_http: None,
                registration,
                project,
                hosting,
            }
        });
        match &entry.latest_http {
            Some(existing) if http.checked_at > existing.checked_at => {
                entry.latest_http = Some(http)
            }
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
                if let Some(group) = groups.get_mut(&root_name) {
                    group.subdomains.push(asset.clone());
                }
                found_parent = true;
                break;
            }
        }

        if !found_parent {
            groups.insert(
                asset.name.clone(),
                DomainGroup {
                    root: asset,
                    subdomains: Vec::new(),
                },
            );
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
            update_stats(&mut stats, sub);
        }
    }

    let asset_tags = crate::app::tags::get_active_tags(&records.asset_tags);

    Ok(TuiModel {
        groups: final_groups,
        stats,
        asset_tags,
    })
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
    use crate::inventory::ItemType;

    #[test]
    fn test_same_root_subdomains() {
        assert!(is_same_root_domain("shop.josera.de", "template.josera.de"));
        assert!(is_same_root_domain("josera.de", "shop.josera.de"));
        assert!(is_same_root_domain("a.b.josera.de", "josera.de"));
    }

    #[test]
    fn test_different_root_domains() {
        assert!(!is_same_root_domain("josera.de", "maxcluster.net"));
        assert!(!is_same_root_domain(
            "phonebook.josera.de",
            "convertexperiments.com"
        ));
    }

    #[test]
    fn test_same_domain_exact() {
        assert!(is_same_root_domain("josera.de", "josera.de"));
    }

    #[test]
    fn test_two_part_tld() {
        assert!(is_same_root_domain(
            "foo.example.co.uk",
            "bar.example.co.uk"
        ));
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
                item_type: ItemType::HttpCheck,
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
            registration: None,
            project: None,
            hosting: None,
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
                item_type: ItemType::HttpCheck,
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
            registration: None,
            project: None,
            hosting: None,
        };
        // Different root domain → IS external
        assert_eq!(
            asset.external_redirect(),
            Some("mailhog.c-1694.maxcluster.net".to_string())
        );
    }

    #[test]
    fn test_external_redirect_www_not_external() {
        let asset = AssetState {
            name: "example.com".to_string(),
            latest_dns: None,
            latest_http: Some(HttpObservation {
                item_type: ItemType::HttpCheck,
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
            registration: None,
            project: None,
            hosting: None,
        };
        // www + subfolder → NOT external
        assert!(asset.external_redirect().is_none());
    }

    #[test]
    fn test_external_redirect_via_chain_fallback() {
        use crate::app::probe::RedirectHop;

        // Simulates josera-estonia.com: final_url is wrongly set to self,
        // but redirect_chain shows a 301 to josera.com/et
        let asset = AssetState {
            name: "josera-estonia.com".to_string(),
            latest_dns: None,
            latest_http: Some(HttpObservation {
                item_type: ItemType::HttpCheck,
                id: "test".to_string(),
                asset_type: "domain".to_string(),
                target: "josera-estonia.com".to_string(),
                status: Some(200),
                response_time_ms: 100,
                final_url: Some("https://josera.com/et".to_string()),
                redirect_chain: vec![RedirectHop {
                    url: "https://josera-estonia.com/".to_string(),
                    status: 301,
                    redirect_to: "https://josera.com/et".to_string(),
                }],
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
            registration: None,
            project: None,
            hosting: None,
        };
        // Different root domain → IS external
        assert_eq!(asset.external_redirect(), Some("josera.com".to_string()));
    }

    #[test]
    fn test_external_redirect_chain_only_no_final_url() {
        use crate::app::probe::RedirectHop;

        // final_url is None, but redirect_chain has the data
        let asset = AssetState {
            name: "josera-estonia.com".to_string(),
            latest_dns: None,
            latest_http: Some(HttpObservation {
                item_type: ItemType::HttpCheck,
                id: "test".to_string(),
                asset_type: "domain".to_string(),
                target: "josera-estonia.com".to_string(),
                status: Some(200),
                response_time_ms: 100,
                final_url: None,
                redirect_chain: vec![RedirectHop {
                    url: "https://josera-estonia.com/".to_string(),
                    status: 301,
                    redirect_to: "https://josera.com/et".to_string(),
                }],
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
            registration: None,
            project: None,
            hosting: None,
        };
        assert_eq!(asset.external_redirect(), Some("josera.com".to_string()));
    }
}

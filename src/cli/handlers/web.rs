use std::fs;
use std::path::Path;
use std::process;
use std::sync::Arc;
use serde::Serialize;

use crate::app::tags::get_junk_assets;
use crate::storage::StorageEngine;
use crate::exit_codes::*;

/// Asset exported for web dashboard
#[derive(Serialize, Debug, Clone)]
pub struct ExportedAsset {
    pub name: String,
    pub is_subdomain: bool,
    pub risk_score: u32,
    pub lifecycle: String,
    pub http_status: Option<u16>,
    pub http_status_text: String,
    pub tech: Vec<String>,
    pub dns: ExportedDns,
    pub http: Option<ExportedHttp>,
    pub tls: Option<ExportedTls>,
    pub hosting: Option<ExportedHosting>,
    pub project: Option<ExportedProject>,
    pub tags: Vec<String>,
    pub last_checked: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct ExportedDns {
    pub a: Vec<String>,
    pub aaaa: Vec<String>,
    pub cname: Vec<String>,
    pub mx: Vec<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct ExportedHttp {
    pub title: Option<String>,
    pub server: Option<String>,
    pub final_url: Option<String>,
    pub redirect_chain: Vec<String>,
    pub response_time_ms: u64,
}

#[derive(Serialize, Debug, Clone)]
pub struct ExportedTls {
    pub issuer: String,
    pub expires_at: String,
    pub days_until_expiry: i64,
}

#[derive(Serialize, Debug, Clone)]
pub struct ExportedHosting {
    pub provider: String,
    pub category: String,
    pub confidence: String,
    pub cdn_detected: bool,
    pub cdn_name: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct ExportedProject {
    pub name: String,
    pub company_id: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct ExportedFinding {
    pub id: String,
    pub severity: String,
    pub category: String,
    pub check_name: String,
    pub target: String,
    pub message: String,
    pub created_at: String,
    pub status: String, // "open" | "acknowledged" | "resolved"
}

#[derive(Serialize, Debug, Clone)]
pub struct ExportedMetadata {
    pub generated_at: String,
    pub total_assets: usize,
    pub total_findings: usize,
    pub open_findings: usize,
    pub critical_findings: usize,
    pub high_risk_assets: usize,
}

pub(crate) fn cmd_web_export(output_dir: &str, storage: &Arc<dyn StorageEngine>) {
    let output_path = Path::new(output_dir);

    // Create output directory
    if let Err(e) = fs::create_dir_all(output_path) {
        eprintln!("Error creating output directory: {}", e);
        process::exit(EXIT_GENERAL_ERROR);
    }

    // Read all data
    let records = match storage.read_all_records() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error reading records: {}", e);
            process::exit(EXIT_STORAGE_ERROR);
        }
    };

    // Build asset indexes
    let junk_set = get_junk_assets(&records.asset_tags);
    let all_assets = build_asset_index(&records, &junk_set, false);
    let clean_assets = build_asset_index(&records, &junk_set, true);

    let assets_json = match serde_json::to_string_pretty(&clean_assets) {
        Ok(json) => json,
        Err(e) => {
            eprintln!("Error serializing assets: {}", e);
            process::exit(EXIT_GENERAL_ERROR);
        }
    };
    if let Err(e) = fs::write(output_path.join("assets.json"), assets_json) {
        eprintln!("Error writing assets.json: {}", e);
        process::exit(EXIT_GENERAL_ERROR);
    }

    let assets_all_json = match serde_json::to_string_pretty(&all_assets) {
        Ok(json) => json,
        Err(e) => {
            eprintln!("Error serializing assets-all: {}", e);
            process::exit(EXIT_GENERAL_ERROR);
        }
    };
    if let Err(e) = fs::write(output_path.join("assets-all.json"), assets_all_json) {
        eprintln!("Error writing assets-all.json: {}", e);
        process::exit(EXIT_GENERAL_ERROR);
    }

    // Build findings index
    let findings = build_findings_index(&records);
    let findings_json = match serde_json::to_string_pretty(&findings) {
        Ok(json) => json,
        Err(e) => {
            eprintln!("Error serializing findings: {}", e);
            process::exit(EXIT_GENERAL_ERROR);
        }
    };
    if let Err(e) = fs::write(output_path.join("findings.json"), findings_json) {
        eprintln!("Error writing findings.json: {}", e);
        process::exit(EXIT_GENERAL_ERROR);
    }

    // Build metadata (use clean asset count for stakeholder-facing metadata)
    let metadata = ExportedMetadata {
        generated_at: chrono::Utc::now().to_rfc3339(),
        total_assets: clean_assets.len(),
        total_findings: findings.len(),
        open_findings: findings.iter().filter(|f| f.status == "open").count(),
        critical_findings: findings.iter().filter(|f| f.severity == "Critical").count(),
        high_risk_assets: clean_assets.iter().filter(|a| a.risk_score >= 100).count(),
    };
    let metadata_json = match serde_json::to_string_pretty(&metadata) {
        Ok(json) => json,
        Err(e) => {
            eprintln!("Error serializing metadata: {}", e);
            process::exit(EXIT_GENERAL_ERROR);
        }
    };
    if let Err(e) = fs::write(output_path.join("metadata.json"), metadata_json) {
        eprintln!("Error writing metadata.json: {}", e);
        process::exit(EXIT_GENERAL_ERROR);
    }

    println!("✓ Exported {} assets ({} clean, {} junk)", all_assets.len(), clean_assets.len(), all_assets.len() - clean_assets.len());
    println!("✓ Exported {} findings", findings.len());
    println!("✓ Output: {}", output_dir);

    process::exit(EXIT_SUCCESS);
}

fn build_asset_index(
    records: &crate::storage::reader::Records,
    junk_set: &std::collections::BTreeSet<String>,
    exclude_junk: bool,
) -> Vec<ExportedAsset> {
    use crate::inventory::item::ItemType;
    use std::collections::HashSet;

    let mut seen = HashSet::new();
    let mut assets = Vec::new();

    for item in &records.items {
        // Only export domains and subdomains
        if item.item_type != ItemType::Domain && item.item_type != ItemType::Subdomain {
            continue;
        }

        // Skip junk assets when exclude_junk is true
        if exclude_junk && junk_set.contains(&item.name) {
            continue;
        }

        // Deduplicate by name — keep first occurrence
        if !seen.insert(item.name.clone()) {
            continue;
        }

        let is_subdomain = item.item_type == ItemType::Subdomain;

        // Get latest HTTP observation
        let http_obs = records
            .http_observations
            .iter()
            .filter(|o| o.target == item.name)
            .max_by_key(|o| o.checked_at);

        // Get latest DNS observation
        let dns_obs = records
            .dns_observations
            .iter()
            .filter(|o| o.target == item.name)
            .max_by_key(|o| o.checked_at);

        // Get latest hosting detection
        let hosting_obs = records
            .hosting_observations
            .iter()
            .filter(|h| h.target == item.name)
            .max_by_key(|h| h.checked_at);

        // Get tags
        let tags: Vec<String> = records
            .asset_tags
            .iter()
            .filter(|t| t.asset_name == item.name && t.removed_at.is_none())
            .map(|t| t.tag.clone())
            .collect();

        // Get lifecycle state
        let lifecycle = get_latest_lifecycle(&item.name, &records.lifecycle_events);

        // Get risk score (placeholder - would need to call calculate_all_risks in real implementation)
        let risk_score = calculate_asset_risk(&item.name, records);

        // Get project context
        let project = get_project_context(&item.name, records);

        assets.push(ExportedAsset {
            name: item.name.clone(),
            is_subdomain,
            risk_score,
            lifecycle,
            http_status: http_obs.and_then(|o| o.status),
            http_status_text: format_http_status(http_obs),
            tech: http_obs
                .map(|o| o.tech.clone())
                .unwrap_or_default(),
            dns: ExportedDns {
                a: dns_obs.map(|o| o.a.clone()).unwrap_or_default(),
                aaaa: dns_obs.map(|o| o.aaaa.clone()).unwrap_or_default(),
                cname: dns_obs.map(|o| o.cname.clone()).unwrap_or_default(),
                mx: dns_obs.map(|o| o.mx.clone()).unwrap_or_default(),
            },
            http: http_obs.map(|o| ExportedHttp {
                title: o.title.clone(),
                server: o.webserver.clone(),
                final_url: o.final_url.clone(),
                redirect_chain: o
                    .redirect_chain
                    .iter()
                    .map(|r| r.url.clone())
                    .collect(),
                response_time_ms: o.response_time_ms,
            }),
            tls: http_obs.and_then(|o| {
                o.tls.as_ref().map(|tls| {
                    let now = chrono::Utc::now();
                    let days_until_expiry = (tls.not_after - now).num_days();
                    ExportedTls {
                        issuer: tls.issuer_dn.clone(),
                        expires_at: tls.not_after.to_rfc3339(),
                        days_until_expiry,
                    }
                })
            }),
            hosting: hosting_obs.map(|h| ExportedHosting {
                provider: h
                    .provider_name
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string()),
                category: h
                    .provider_category
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string()),
                confidence: format!("{:?}", h.confidence),
                cdn_detected: h.cdn_detected,
                cdn_name: h.cdn_name.clone(),
            }),
            project,
            tags,
            last_checked: http_obs
                .map(|o| o.checked_at.to_rfc3339())
                .unwrap_or_default(),
        });
    }

    // Sort by risk score descending, then name
    assets.sort_by(|a, b| {
        b.risk_score
            .cmp(&a.risk_score)
            .then_with(|| a.name.cmp(&b.name))
    });

    assets
}

fn build_findings_index(records: &crate::storage::reader::Records) -> Vec<ExportedFinding> {
    use crate::app::finding::FindingEventType;

    records
        .findings
        .iter()
        .map(|f| {
            // Determine status
            let is_resolved = records
                .finding_events
                .iter()
                .any(|e| e.finding_id == f.id && e.event_type == FindingEventType::Resolved);
            let is_acknowledged = records
                .finding_events
                .iter()
                .any(|e| e.finding_id == f.id && e.event_type == FindingEventType::Acknowledged);

            let status = if is_resolved {
                "resolved".to_string()
            } else if is_acknowledged {
                "acknowledged".to_string()
            } else {
                "open".to_string()
            };

            ExportedFinding {
                id: f.id.clone(),
                severity: format!("{:?}", f.severity),
                category: format!("{:?}", f.category),
                check_name: f.check_name.clone(),
                target: f.target.clone(),
                message: f.message.clone(),
                created_at: f.created_at.to_rfc3339(),
                status,
            }
        })
        .collect()
}

fn format_http_status(obs: Option<&crate::app::probe::types::HttpObservation>) -> String {
    match obs {
        Some(o) => {
            if let Some(status) = o.status {
                format!("{}", status)
            } else if o.error_message.is_some() {
                "Error".to_string()
            } else {
                "Timeout".to_string()
            }
        }
        None => "Not checked".to_string(),
    }
}

fn get_latest_lifecycle(
    asset_name: &str,
    lifecycle_events: &[crate::inventory::lifecycle::LifecycleEvent],
) -> String {
    lifecycle_events
        .iter()
        .filter(|e| e.asset_name == asset_name)
        .max_by_key(|e| e.created_at)
        .map(|e| e.to_state.to_string())
        .unwrap_or_else(|| "Unknown".to_string())
}

fn get_project_context(
    asset_name: &str,
    records: &crate::storage::reader::Records,
) -> Option<ExportedProject> {
    records
        .asset_assignments
        .iter()
        .find(|a| a.host == asset_name && !a.unassigned)
        .and_then(|assignment| {
            records
                .projects
                .iter()
                .find(|p| p.id == assignment.project)
                .map(|project| ExportedProject {
                    name: project.name.clone(),
                    company_id: format!("{:?}", project.company),
                })
        })
}

fn calculate_asset_risk(asset_name: &str, records: &crate::storage::reader::Records) -> u32 {
    // Simple risk calculation based on findings
    // In production, would use actual risk scoring from crate::app::risk
    let critical_findings = records
        .findings
        .iter()
        .filter(|f| f.target == asset_name)
        .filter(|f| {
            use crate::app::finding::FindingSeverity;
            f.severity == FindingSeverity::Critical
        })
        .count();

    let high_findings = records
        .findings
        .iter()
        .filter(|f| f.target == asset_name)
        .filter(|f| {
            use crate::app::finding::FindingSeverity;
            f.severity == FindingSeverity::High
        })
        .count();

    (critical_findings as u32 * 50) + (high_findings as u32 * 25)
}

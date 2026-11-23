//! CLI handlers for network service discovery, application reference
//! extraction, and candidate evidence explanation (ADRs 0026–0031).

use std::fs;
use std::sync::Arc;

use chrono::Utc;

use crate::app::discovery::run_autonomous_discovery;
use crate::app::external_services::{
    correlate_service_observations, parse_censys_host_json, parse_shodan_host_json,
};
use crate::app::references::{
    extract_dns_srv_txt_references, extract_js_references, parse_openapi_references,
    parse_public_repo_references, parse_urlscan_result_json,
};
use crate::app::relationships::related_entities;
use crate::domain::normalize::normalize_fqdn;
use crate::domain::service::{ApplicationReferenceObservation, ServiceObservation};
use crate::error::{BalebosError, Result};
use crate::storage::{StorageEngine, StorageEngineExt};

/// List persisted network services (`balebos services list`).
pub fn cmd_services_list(json: bool, storage: &Arc<dyn StorageEngine>) -> Result<()> {
    let records = storage.read_all_records()?;
    let correlated = correlate_service_observations(&records.service_observations);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&correlated)
                .map_err(|e| BalebosError::Validation(e.to_string()))?
        );
        return Ok(());
    }
    if correlated.is_empty() {
        println!("No network service observations persisted.");
        return Ok(());
    }
    println!("Discovered network services ({}):", correlated.len());
    for svc in &correlated {
        let proto = svc
            .protocol_id
            .protocol()
            .map(|p| p.to_string())
            .unwrap_or_else(|| "unidentified".to_string());
        let ts = svc
            .source_recorded_at
            .map(|t| t.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "unknown".to_string());
        println!(
            "  {} [{}] proto={} state={} verification={} source={} recorded={} scope={}",
            svc.endpoint,
            svc.category,
            proto,
            svc.state,
            svc.verification,
            svc.source,
            ts,
            if svc.in_scope {
                "in-scope"
            } else {
                "out-of-scope"
            }
        );
    }
    Ok(())
}

/// Show detailed evidence for one service endpoint (`balebos services show <endpoint>`).
pub fn cmd_services_show(
    endpoint: &str,
    json: bool,
    storage: &Arc<dyn StorageEngine>,
) -> Result<()> {
    let records = storage.read_all_records()?;
    let target = endpoint.trim().to_ascii_lowercase();
    let matches: Vec<&ServiceObservation> = records
        .service_observations
        .iter()
        .filter(|s| {
            s.endpoint.canonical_id() == target
                || s.host_ip.as_deref() == Some(target.as_str())
                || s.associated_hosts.iter().any(|h| h == &target)
        })
        .collect();

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&matches)
                .map_err(|e| BalebosError::Validation(e.to_string()))?
        );
        return Ok(());
    }
    if matches.is_empty() {
        println!("No service observations found for '{}'.", endpoint);
        return Ok(());
    }
    for svc in matches {
        println!("Service: {}", svc.endpoint);
        println!("  Category:     {}", svc.category);
        println!("  State:        {}", svc.state);
        println!("  Protocol:     {:?}", svc.protocol_id);
        println!("  Verification: {}", svc.verification);
        println!("  Source:       {}", svc.source);
        if let Some(ts) = svc.source_recorded_at {
            println!("  Observed at:  {}", ts.to_rfc3339());
        }
        println!("  Ingested at:  {}", svc.ingested_at.to_rfc3339());
        println!(
            "  Scope:        {}",
            if svc.in_scope {
                "Authorised scope"
            } else {
                "Active verification not authorised"
            }
        );
        if let Some(iface) = &svc.interface_name {
            println!("  Interface:    {}", iface);
        }
        if !svc.fingerprints.is_empty() {
            println!("  Fingerprints:");
            for fp in &svc.fingerprints {
                println!("    - {:?} ({:?}): {}", fp.kind, fp.power, fp.value);
            }
        }
        if !svc.evidence.is_empty() {
            println!("  Evidence:");
            for ev in &svc.evidence {
                println!("    - {}", ev);
            }
        }
        if let Some(unc) = &svc.uncertainty {
            println!("  Uncertainty:  {}", unc);
        }
    }
    Ok(())
}

/// Ingest external service intelligence from offline Censys/Shodan JSON
/// (`balebos discover services <target> --censys <file> | --shodan <file>`).
pub fn cmd_discover_services(
    target: &str,
    provider: &str,
    input_file: Option<&str>,
    scope_domains: &[String],
    save: bool,
    storage: &Arc<dyn StorageEngine>,
) -> Result<()> {
    let path = input_file.ok_or_else(|| {
        BalebosError::Validation(
            "Offline service discovery requires --input <file> (Censys or Shodan JSON fixture)"
                .to_string(),
        )
    })?;
    let raw = fs::read_to_string(path)?;
    let now = Utc::now();
    let parsed = match provider.to_ascii_lowercase().as_str() {
        "censys" => parse_censys_host_json(&raw, scope_domains, now)?,
        "shodan" => parse_shodan_host_json(&raw, scope_domains, now)?,
        other => {
            return Err(BalebosError::Validation(format!(
                "Unsupported service intelligence provider '{}'; expected 'censys' or 'shodan'",
                other
            )));
        }
    };

    println!(
        "Parsed {} service observation(s) for '{}' via {}:",
        parsed.len(),
        target,
        provider
    );
    for svc in &parsed {
        if save {
            storage.save(svc)?;
        }
        println!(
            "  {} [{}] proto={:?} scope={}",
            svc.endpoint, svc.category, svc.protocol_id, svc.in_scope
        );
    }
    Ok(())
}

/// Extract application & infrastructure references (`balebos discover references <domain>`).
#[allow(clippy::too_many_arguments)]
pub fn cmd_discover_references(
    domain: &str,
    kind: &str,
    input_file: Option<&str>,
    urlscan_submitted: bool,
    scope_domains: &[String],
    save: bool,
    storage: &Arc<dyn StorageEngine>,
) -> Result<()> {
    let now = Utc::now();
    let norm = normalize_fqdn(domain);
    let refs: Vec<ApplicationReferenceObservation> = match kind.to_ascii_lowercase().as_str() {
        "dns" => {
            let records = storage.read_all_records()?;
            let mut txts = Vec::new();
            let mut srvs = Vec::new();
            for dns in &records.dns_observations {
                if normalize_fqdn(&dns.target) == norm {
                    txts.extend(dns.txt.clone());
                    srvs.extend(dns.srv.clone());
                }
            }
            extract_dns_srv_txt_references(&norm, &srvs, &txts, scope_domains, now)
        }
        "js" => {
            let path = input_file.ok_or_else(|| {
                BalebosError::Validation("--kind js requires --input <file>".to_string())
            })?;
            let content = fs::read_to_string(path)?;
            extract_js_references(&norm, path, &content, scope_domains, None, now)
        }
        "openapi" => {
            let path = input_file.ok_or_else(|| {
                BalebosError::Validation("--kind openapi requires --input <file>".to_string())
            })?;
            let content = fs::read_to_string(path)?;
            parse_openapi_references(&norm, path, &content, scope_domains, None, now)?
        }
        "urlscan" => {
            let path = input_file.ok_or_else(|| {
                BalebosError::Validation("--kind urlscan requires --input <file>".to_string())
            })?;
            let content = fs::read_to_string(path)?;
            parse_urlscan_result_json(&norm, &content, urlscan_submitted, scope_domains, now)?
        }
        "repo" => {
            let path = input_file.ok_or_else(|| {
                BalebosError::Validation("--kind repo requires --input <file>".to_string())
            })?;
            let content = fs::read_to_string(path)?;
            parse_public_repo_references(&norm, path, &content, scope_domains, None, now)
        }
        other => {
            return Err(BalebosError::Validation(format!(
                "Unsupported reference kind '{}'; expected dns|js|openapi|urlscan|repo",
                other
            )));
        }
    };

    println!(
        "Extracted {} application reference(s) for '{}' ({}):",
        refs.len(),
        norm,
        kind
    );
    for r in &refs {
        if save {
            storage.save(r)?;
        }
        println!(
            "  {} -> {} ({}) [{}] from {}",
            r.origin_asset,
            r.referenced_target,
            r.relationship_kind,
            r.verification,
            r.discovered_in
        );
    }
    Ok(())
}

/// Explain all evidence and relationships supporting a candidate
/// (`balebos candidates explain <candidate>`).
pub fn cmd_candidates_explain(
    candidate: &str,
    json: bool,
    storage: &Arc<dyn StorageEngine>,
) -> Result<()> {
    let records = storage.read_all_records()?;
    let target = candidate.trim().to_ascii_lowercase();
    let services: Vec<&ServiceObservation> = records
        .service_observations
        .iter()
        .filter(|s| s.endpoint.canonical_id() == target)
        .collect();
    let refs: Vec<&ApplicationReferenceObservation> = records
        .application_references
        .iter()
        .filter(|r| r.referenced_target.to_ascii_lowercase() == target)
        .collect();
    let rels = related_entities(&records, &target);

    if json {
        let payload = serde_json::json!({
            "candidate": target,
            "services": services,
            "references": refs,
            "direct_relationships": rels.direct,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload)
                .map_err(|e| BalebosError::Validation(e.to_string()))?
        );
        return Ok(());
    }

    println!("Candidate: {}", target);
    for svc in &services {
        println!("Type: NetworkService ({})", svc.category);
        println!("Protocol: {:?}", svc.protocol_id);
        println!("Evidence:");
        println!("  Source: {}", svc.source);
        if let Some(ts) = svc.source_recorded_at {
            println!("  Observed: {}", ts.format("%Y-%m-%d"));
        }
        println!("Verification: {}", svc.verification);
        println!(
            "Scope: {}",
            if svc.in_scope {
                "Authorised"
            } else {
                "Active verification not authorised"
            }
        );
    }
    for r in &refs {
        println!("Type: {} ({})", r.target_kind, r.reference_type);
        println!("  Origin: {}", r.origin_asset);
        println!("  Discovered in: {}", r.discovered_in);
        println!("  Verification: {}", r.verification);
        if r.deprecated_hint {
            println!("  Hint: potentially deprecated");
        }
        if r.dev_or_placeholder_hint {
            println!("  Hint: development/placeholder reference");
        }
    }
    if !rels.direct.is_empty() {
        println!("Relationships:");
        for edge in &rels.direct {
            println!(
                "  {} --{}--> {} [{}]",
                edge.source, edge.kind, edge.target, edge.signal
            );
        }
    }
    if services.is_empty() && refs.is_empty() && rels.direct.is_empty() {
        println!("No evidence found for candidate '{}'.", candidate);
    }
    Ok(())
}

/// Run the autonomous bounded discovery pipeline (`balebos discover run <domain>`).
pub fn cmd_discover_run(
    domain: &str,
    scope_domains: &[String],
    save: bool,
    storage: &Arc<dyn StorageEngine>,
) -> Result<()> {
    let summary = run_autonomous_discovery(domain, scope_domains, save, storage)?;
    println!("Autonomous discovery for '{}':", summary.domain);
    println!("  CT candidates:      {}", summary.ct_candidates);
    println!("  DNS SRV/TXT refs:   {}", summary.dns_references);
    println!(
        "  JS assets / refs:   {} / {}",
        summary.js_assets_fetched, summary.js_references
    );
    println!("  OpenAPI refs:       {}", summary.openapi_references);
    if !summary.errors.is_empty() {
        println!("  Partial errors ({}):", summary.errors.len());
        for err in &summary.errors {
            println!("    - {}", err);
        }
    }
    Ok(())
}

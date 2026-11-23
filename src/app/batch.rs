use crate::app::probe;
use crate::domain::normalize_domain;
use crate::error::Result;
use crate::hosting;
use crate::storage::{StorageEngine, StorageEngineExt};
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

/// Statistics gathered during batch processing
#[derive(Debug, Clone)]
pub struct BatchStats {
    pub domains_added: usize,
    pub domains_failed: usize,
    pub subdomains_found: usize,
    pub dns_checks_completed: usize,
    pub http_checks_completed: usize,
    pub http_errors: usize,
    pub total_runtime_secs: u64,
}

/// Load domains from a text file (one domain per line)
fn load_domains_from_file(filepath: &str) -> Result<Vec<String>> {
    let path = Path::new(filepath);

    if !path.exists() {
        return Err(crate::error::BalebosError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found: {}", filepath),
        )));
    }

    let file = File::open(filepath)?;
    let reader = BufReader::new(file);

    let mut domains = Vec::new();

    for line_result in reader.lines() {
        let line = line_result?;
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Normalize the domain
        let normalized = normalize_domain(trimmed);
        if !normalized.is_empty() {
            domains.push(normalized);
        }
    }

    Ok(domains)
}

/// Pipeline result for a single domain group
#[derive(Debug, Clone)]
struct GroupPipelineResult {
    subdomains: Vec<String>,
    sub_items: Vec<crate::inventory::InventoryItem>,
    dns_obs: Vec<probe::DnsObservation>,
    http_obs: Vec<probe::HttpObservation>,
    junk_candidates: Vec<String>,
}

/// Run full reconnaissance on a batch of domains with per-group pipelining
pub fn batch_process(
    filepath: &str,
    storage: &Arc<dyn StorageEngine>,
    ignore_patterns: &[String],
    allow_internal: bool,
) -> Result<BatchStats> {
    let start = Instant::now();
    let started_at = chrono::Utc::now();

    // Load domains from file
    println!("📂 Loading domains from: {}", filepath);
    let domains = load_domains_from_file(filepath)?;
    println!("✓ Loaded {} domains\n", domains.len());

    // Load junk assets for filtering
    let junk_assets: std::collections::BTreeSet<String> = storage
        .read_all_records()
        .map(|r| crate::app::tags::get_junk_assets(&r.asset_tags))
        .unwrap_or_default();

    let mut stats = BatchStats {
        domains_added: 0,
        domains_failed: 0,
        subdomains_found: 0,
        dns_checks_completed: 0,
        http_checks_completed: 0,
        http_errors: 0,
        total_runtime_secs: 0,
    };

    // Load existing records to skip RDAP if already checked today
    let existing_records = storage.read_all_records().ok();
    let checked_today: std::collections::HashSet<String> = existing_records
        .as_ref()
        .map(|r| {
            let now = chrono::Utc::now().date_naive();
            r.domain_registrations
                .iter()
                .filter(|reg| reg.created_at.date_naive() == now)
                .map(|reg| reg.domain.clone())
                .collect()
        })
        .unwrap_or_default();

    // ========================================================================
    // Step 1: Scan all domains (Parallel — already done with rayon)
    // ========================================================================
    println!("🔍 Step 1/7: Scanning domains and RDAP info (parallel)...");

    let results: Vec<_> = domains
        .par_iter()
        .enumerate()
        .map(|(idx, domain)| {
            if let Err(denial) = crate::app::probe::scope::check_scope(domain, allow_internal) {
                println!(
                    "  ⊘ [{}/{}] {} (out of scope: {})",
                    idx + 1,
                    domains.len(),
                    domain,
                    denial
                );
                return (
                    idx,
                    domain.clone(),
                    None,
                    None,
                    Some(format!("out of scope: {denial}")),
                    false,
                );
            }
            if crate::ignore::is_ignored(domain, ignore_patterns)
                || crate::ignore::is_junk(domain, &junk_assets)
            {
                return (idx, domain.clone(), None, None, None, true);
            }

            let skip_rdap = checked_today.contains(domain);
            match crate::app::scan::scan_domain_no_save(domain) {
                Ok((_, item)) => {
                    let rdap_result = if skip_rdap {
                        None
                    } else {
                        probe::run_rdap_probe_no_save(domain).ok()
                    };
                    (
                        idx,
                        domain.clone(),
                        Some(item),
                        rdap_result,
                        None,
                        skip_rdap,
                    )
                }
                Err(e) => (idx, domain.clone(), None, None, Some(e.to_string()), false),
            }
        })
        .collect();

    // Save results and print status sequentially to avoid jumbled output
    let mut new_items = Vec::new();
    let mut new_rdaps = Vec::new();
    let mut skipped_rdap_count = 0;
    // Per-target coverage for the persisted ScanReport (ADR 0005).
    let mut coverage_outcomes: Vec<crate::domain::coverage::TargetCoverageRecord> = Vec::new();

    for (idx, domain, item_opt, rdap_opt, err_opt, _skip_rdap) in results {
        if let Some(err) = err_opt {
            if let Some(denial) = err.strip_prefix("out of scope: ") {
                // Scope refusals are deliberate skips, not failures.
                println!(
                    "  ⊘ [{}/{}] {} ({})",
                    idx + 1,
                    domains.len(),
                    domain,
                    denial
                );
                coverage_outcomes.push(crate::domain::coverage::TargetCoverageRecord {
                    target: domain.clone(),
                    coverage: crate::domain::coverage::TargetCoverage::Skipped,
                });
                continue;
            }
            println!(
                "  ✗ [{}/{}] {} (error: {})",
                idx + 1,
                domains.len(),
                domain,
                err
            );
            stats.domains_failed += 1;
            coverage_outcomes.push(crate::domain::coverage::TargetCoverageRecord {
                target: domain.clone(),
                coverage: crate::domain::coverage::TargetCoverage::Failed { error: err },
            });
        } else if let Some(item) = item_opt {
            let status = if _skip_rdap {
                skipped_rdap_count += 1;
                "(RDAP fresh, skipped)"
            } else if rdap_opt.is_some() {
                "(RDAP updated)"
            } else {
                "(RDAP failed/no data)"
            };
            println!("  ✓ [{}/{}] {} {}", idx + 1, domains.len(), domain, status);
            stats.domains_added += 1;
            coverage_outcomes.push(crate::domain::coverage::TargetCoverageRecord {
                target: domain.clone(),
                coverage: crate::domain::coverage::TargetCoverage::Observed,
            });
            new_items.push(item);
            if let Some(rdap) = rdap_opt {
                new_rdaps.push(rdap);
            }
        } else {
            // Deliberately not inspected: ignore pattern or junk asset.
            coverage_outcomes.push(crate::domain::coverage::TargetCoverageRecord {
                target: domain.clone(),
                coverage: crate::domain::coverage::TargetCoverage::Skipped,
            });
        }
    }

    if skipped_rdap_count > 0 {
        println!(
            "  ℹ Skipped {} RDAP checks (already up to date today)",
            skipped_rdap_count
        );
    }

    if !new_items.is_empty() {
        storage.save_batch(&new_items)?;
    }
    if !new_rdaps.is_empty() {
        storage.save_batch(&new_rdaps)?;
    }
    println!();

    // ========================================================================
    // Step 2-4: Pipeline per domain group (Parallel)
    // ========================================================================
    // Each group runs: subfinder → dnsx → httpx sequentially,
    // but groups run in parallel via rayon.
    //
    // This overlaps slow steps across groups:
    //   Group 1: [subfinder]→[dnsx]→[httpx]
    //   Group 2:          [subfinder]→[dnsx]→[httpx]
    //   Group 3:                   [subfinder]→[dnsx]→[httpx]
    //
    // Without pipeline, we wait for ALL subfinders before ANY dnsx starts.

    let group_size = 10;
    let groups: Vec<Vec<String>> = domains
        .chunks(group_size)
        .map(|chunk| chunk.to_vec())
        .collect();

    println!(
        "🔄 Steps 2-4: Pipelined reconnaissance ({} groups of ~{} domains, parallel)...",
        groups.len(),
        group_size
    );

    // Collect all results in memory, save to storage once at the end.
    // This avoids 3×N save_batch calls per group (Parquet I/O is expensive).
    let pipeline_results: Vec<GroupPipelineResult> = groups
        .par_iter()
        .enumerate()
        .filter_map(|(group_idx, group_domains)| {
            let group_num = group_idx + 1;

            // Log group start
            println!(
                "  📊 Group {}/{} starting ({} domains)...",
                group_num,
                groups.len(),
                group_domains.len()
            );

            // Filter ignored and junk domains for this group
            let valid_domains: Vec<String> = group_domains
                .iter()
                .filter(|d| {
                    !crate::ignore::is_ignored(d, ignore_patterns)
                        && !crate::ignore::is_junk(d, &junk_assets)
                })
                .cloned()
                .collect();

            if valid_domains.is_empty() {
                println!("  ⏭ Group {} skipped (all domains ignored/junk)", group_num);
                return None;
            }

            // --- Subfinder (no save) ---
            println!(
                "  🔍 Group {}/{}: Running subfinder on {} domains...",
                group_num,
                groups.len(),
                valid_domains.len()
            );
            let (subdomains, sub_items) =
                match probe::probe_subdomains_batch_no_save(&valid_domains) {
                    Ok((subs, items)) => {
                        println!(
                            "  ✅ Group {}/{}: Found {} subdomains",
                            group_num,
                            groups.len(),
                            subs.len()
                        );
                        (subs, items)
                    }
                    Err(e) => {
                        eprintln!("  ✗ Group {} subfinder failed: {}", group_num, e);
                        (vec![], vec![])
                    }
                };

            // Detect junk candidates from this group's results
            let junk_candidates = probe::detect_junk_candidates(&subdomains, &valid_domains);

            // Build targets: domains + subdomains
            let mut targets = valid_domains.clone();
            for sub in &subdomains {
                if !crate::ignore::is_ignored(sub, ignore_patterns)
                    && !crate::ignore::is_junk(sub, &junk_assets)
                {
                    targets.push(sub.clone());
                }
            }

            // --- DNS (no save) ---
            let dns_obs = match probe::probe_dns_batch_no_save(&targets) {
                Ok(obs) => obs,
                Err(e) => {
                    eprintln!("  ✗ Group {} DNS failed: {}", group_num, e);
                    vec![]
                }
            };

            // Filter HTTP targets: only those with DNS records
            let has_dns: std::collections::HashSet<String> = dns_obs
                .iter()
                .filter(|o| {
                    !o.a.is_empty() || !o.aaaa.is_empty() || !o.cname.is_empty() || !o.mx.is_empty()
                })
                .map(|o| o.target.clone())
                .collect();

            let http_targets: Vec<String> = targets
                .into_iter()
                .filter(|t| has_dns.contains(t))
                .collect();

            // --- HTTP (no save) ---
            let http_obs = if http_targets.is_empty() {
                vec![]
            } else {
                match probe::probe_http_batch_no_save(&http_targets) {
                    Ok(obs) => obs,
                    Err(e) => {
                        eprintln!("  ✗ Group {} HTTP failed: {}", group_num, e);
                        vec![]
                    }
                }
            };

            // Collect results for this group
            Some(GroupPipelineResult {
                subdomains,
                sub_items,
                dns_obs,
                http_obs,
                junk_candidates,
            })
        })
        .collect();

    // Aggregate stats and collect all items for single save
    let mut all_items = Vec::new();
    let mut all_dns_obs = Vec::new();
    let mut all_http_obs = Vec::new();
    let mut all_junk_candidates: Vec<String> = Vec::new();

    for result in &pipeline_results {
        stats.subdomains_found += result.subdomains.len();
        stats.dns_checks_completed += result.dns_obs.len();
        all_items.extend(result.sub_items.clone());
        all_dns_obs.extend(result.dns_obs.clone());
        all_junk_candidates.extend(result.junk_candidates.clone());

        for obs in &result.http_obs {
            if obs.status.is_some() {
                stats.http_checks_completed += 1;
            } else if obs.error_kind.is_some() {
                stats.http_errors += 1;
            }
        }
        all_http_obs.extend(result.http_obs.clone());
    }

    // Report junk candidates
    if !all_junk_candidates.is_empty() {
        all_junk_candidates.sort();
        all_junk_candidates.dedup();
        println!(
            "\n⚠️  {} auto-junk candidates detected:",
            all_junk_candidates.len()
        );
        for candidate in &all_junk_candidates {
            println!("   • {}", candidate);
        }
        println!("\n   To review: balebos asset list-junk-candidates");
        println!("   To mark all as junk: balebos asset junk <name>");
    }

    // Single bulk save to storage — eliminates N× save_batch calls
    println!(
        "💾 Saving {} observations to storage...",
        all_items.len() + all_dns_obs.len() + all_http_obs.len()
    );
    if !all_items.is_empty() {
        storage.save_batch(&all_items)?;
    }
    if !all_dns_obs.is_empty() {
        storage.save_batch(&all_dns_obs)?;
    }
    if !all_http_obs.is_empty() {
        storage.save_batch(&all_http_obs)?;
    }

    println!(
        "  ✓ Pipeline complete: {} subdomains, {} DNS, {} HTTP checks\n",
        stats.subdomains_found, stats.dns_checks_completed, stats.http_checks_completed
    );

    // ========================================================================
    // Step 5: Probe ASN for all unique IPs (uses DNS from pipeline)
    // ========================================================================
    println!("🔍 Step 5/7: Looking up ASN information...");

    let ip_to_host = probe::asn::build_ip_host_map(&all_dns_obs);
    let mut asn_by_ip: HashMap<String, probe::AsnObservation> = HashMap::new();

    if ip_to_host.is_empty() {
        println!("  → No IPs to look up");
    } else {
        print!("  \u{27F3} Looking up {} unique IPs ", ip_to_host.len());
        use std::io::Write;
        let _ = std::io::stdout().flush();

        match probe::probe_asn_batch(&ip_to_host, storage) {
            Ok(observations) => {
                println!(" \u{2713}");
                println!("  → Recorded {} ASN records", observations.len());
                for obs in observations {
                    asn_by_ip.insert(obs.target.clone(), obs);
                }
            }
            Err(e) => {
                println!(" \u{2717}");
                println!("  → ASN lookup failed: {} (is asnmap installed?)", e);
            }
        }
    }
    println!();

    // ========================================================================
    // Step 6: Detect hosting providers
    // ========================================================================
    println!("🏠 Step 6/7: Detecting hosting providers...");

    let all_assets = get_all_assets(storage)?;

    let mut dns_map: HashMap<String, probe::DnsObservation> = HashMap::new();
    for obs in &all_dns_obs {
        dns_map
            .entry(obs.target.clone())
            .or_insert_with(|| obs.clone());
    }

    let mut http_map: HashMap<String, probe::HttpObservation> = HashMap::new();
    for obs in &all_http_obs {
        http_map
            .entry(obs.target.clone())
            .or_insert_with(|| obs.clone());
    }

    if all_assets.is_empty() {
        println!("  → No targets to analyze");
    } else {
        print!("  \u{27F3} Analyzing {} targets ", all_assets.len());
        use std::io::Write;
        let _ = std::io::stdout().flush();

        match hosting::detect_hosting_batch(&all_assets, &dns_map, &http_map, &asn_by_ip, storage) {
            Ok(observations) => {
                println!(" \u{2713}");

                let detected_count = observations
                    .iter()
                    .filter(|o| o.provider_id.is_some())
                    .count();
                let cdn_count = observations.iter().filter(|o| o.cdn_detected).count();

                println!(
                    "  → Analyzed {} assets: {} providers identified, {} behind CDN",
                    observations.len(),
                    detected_count,
                    cdn_count
                );
            }
            Err(e) => {
                println!(" \u{2717}");
                println!("  → Hosting detection failed: {}", e);
            }
        }
    }
    println!();

    // Calculate stats
    stats.total_runtime_secs = start.elapsed().as_secs();

    // Print summary
    print_summary(&stats, &domains);

    // Persist scan coverage (ADR 0005): what was planned, what happened.
    // Failed coverage is recorded as failed — it never implies disappearance.
    let report = crate::domain::coverage::ScanReport::start(started_at).finish(
        domains.clone(),
        coverage_outcomes,
        chrono::Utc::now(),
    );
    let observed = report.count_coverage(&crate::domain::coverage::TargetCoverage::Observed);
    let failed = report.count_coverage(&crate::domain::coverage::TargetCoverage::Failed {
        error: String::new(),
    });
    let skipped = report.count_coverage(&crate::domain::coverage::TargetCoverage::Skipped);

    let record = ScanReportRecord {
        item_type: crate::inventory::ItemType::ScanReport,
        report: report.clone(),
    };
    if let Err(e) = storage.save(&record) {
        eprintln!(
            "Warning: failed to persist scan report {}: {}",
            report.scan_id, e
        );
    }
    println!(
        "🧭 Scan report {}: {} planned, {} observed, {} failed, {} skipped",
        report.scan_id,
        report.planned_targets.len(),
        observed,
        failed,
        skipped
    );

    Ok(stats)
}

/// Persisted form of a scan report: the storage discriminator is an
/// application-layer wrapper (ADR 0004) so the domain type stays pure.
#[derive(serde::Serialize)]
struct ScanReportRecord {
    item_type: crate::inventory::ItemType,
    #[serde(flatten)]
    report: crate::domain::coverage::ScanReport,
}

/// Result of a single recon step
#[derive(Debug, Clone)]
pub struct ReconStepResult {
    pub name: &'static str,
    pub success: bool,
    pub message: String,
}

/// Run full reconnaissance on a single domain with progress reporting
pub fn run_single_recon_with_progress(
    domain: &str,
    storage: &Arc<dyn StorageEngine>,
    ignore_patterns: &[String],
    on_step: &dyn Fn(&str),
    on_result: &dyn Fn(&ReconStepResult),
) -> crate::error::Result<Vec<ReconStepResult>> {
    let junk_assets: std::collections::BTreeSet<String> = storage
        .read_all_records()
        .map(|r| crate::app::tags::get_junk_assets(&r.asset_tags))
        .unwrap_or_default();

    if crate::ignore::is_ignored(domain, ignore_patterns)
        || crate::ignore::is_junk(domain, &junk_assets)
    {
        let result = ReconStepResult {
            name: "ignore",
            success: true,
            message: format!("Domain {} ignored by filter", domain),
        };
        on_result(&result);
        return Ok(vec![result]);
    }

    let mut results = Vec::new();

    // 1. Scan/Add domain
    on_step("Scanning domain...");
    match crate::app::scan::scan_domain(domain, storage) {
        Ok(_) => {
            let r = ReconStepResult {
                name: "scan",
                success: true,
                message: format!("Domain {} added to inventory", domain),
            };
            on_result(&r);
            results.push(r);
        }
        Err(e) => {
            let r = ReconStepResult {
                name: "scan",
                success: false,
                message: format!("Failed to scan {}: {}", domain, e),
            };
            on_result(&r);
            results.push(r);
            return Ok(results);
        }
    }

    // 2. Run RDAP probe (apex domain)
    on_step("Probing RDAP registration...");
    match probe::run_rdap_probe(storage, domain) {
        Ok(_) => {
            let r = ReconStepResult {
                name: "rdap",
                success: true,
                message: "RDAP probe completed".to_string(),
            };
            on_result(&r);
            results.push(r);
        }
        Err(e) => {
            let r = ReconStepResult {
                name: "rdap",
                success: false,
                message: format!("RDAP probe failed: {}", e),
            };
            on_result(&r);
            results.push(r);
        }
    }

    // 3. Discover subdomains
    on_step("Discovering subdomains...");
    let subdomains = match probe::probe_subdomains(domain, storage) {
        Ok(subs) => {
            let r = ReconStepResult {
                name: "subdomains",
                success: true,
                message: format!("Found {} subdomains", subs.len()),
            };
            on_result(&r);
            results.push(r);
            subs
        }
        Err(e) => {
            let r = ReconStepResult {
                name: "subdomains",
                success: false,
                message: format!("Subdomain discovery failed: {}", e),
            };
            on_result(&r);
            results.push(r);
            vec![]
        }
    };

    // 4. Prepare targets for batch probes (domain + subdomains)
    let mut targets = vec![domain.to_string()];
    for sub in subdomains {
        if !crate::ignore::is_ignored(&sub, ignore_patterns)
            && !crate::ignore::is_junk(&sub, &junk_assets)
        {
            targets.push(sub);
        }
    }

    // 5. Batch DNS Probe
    on_step("Probing DNS records...");
    let dns_observations = match probe::probe_dns_batch(&targets, storage) {
        Ok(obs) => {
            let r = ReconStepResult {
                name: "dns",
                success: true,
                message: format!("DNS resolved for {} targets", obs.len()),
            };
            on_result(&r);
            results.push(r);
            obs
        }
        Err(e) => {
            let r = ReconStepResult {
                name: "dns",
                success: false,
                message: format!("DNS probe failed: {}", e),
            };
            on_result(&r);
            results.push(r);
            vec![]
        }
    };

    // 6. Batch HTTP Probe (skip targets without DNS records)
    let has_dns: std::collections::HashSet<String> = dns_observations
        .iter()
        .filter(|o| {
            !o.a.is_empty() || !o.aaaa.is_empty() || !o.cname.is_empty() || !o.mx.is_empty()
        })
        .map(|o| o.target.clone())
        .collect();

    let http_targets: Vec<String> = targets
        .iter()
        .filter(|t| has_dns.contains(*t))
        .cloned()
        .collect();

    let skipped = targets.len().saturating_sub(http_targets.len());

    on_step("Probing HTTP status...");
    let http_observations = if http_targets.is_empty() {
        let r = ReconStepResult {
            name: "http",
            success: true,
            message: format!("No targets with DNS records (skipped {})", skipped),
        };
        on_result(&r);
        results.push(r);
        vec![]
    } else {
        match probe::probe_http_batch(&http_targets, storage) {
            Ok(obs) => {
                let success_count = obs.iter().filter(|o| o.status.is_some()).count();
                let r = ReconStepResult {
                    name: "http",
                    success: true,
                    message: format!(
                        "HTTP checked {} targets ({} responded, {} skipped without DNS)",
                        obs.len(),
                        success_count,
                        skipped
                    ),
                };
                on_result(&r);
                results.push(r);
                obs
            }
            Err(e) => {
                let r = ReconStepResult {
                    name: "http",
                    success: false,
                    message: format!("HTTP probe failed: {}", e),
                };
                on_result(&r);
                results.push(r);
                vec![]
            }
        }
    };

    // 7. ASN Probe
    on_step("Looking up ASN information...");
    let ip_to_host = probe::asn::build_ip_host_map(&dns_observations);
    let asn_by_ip: HashMap<String, probe::AsnObservation> = HashMap::new();
    if ip_to_host.is_empty() {
        let r = ReconStepResult {
            name: "asn",
            success: true,
            message: "No IPs to look up (skipping ASN)".to_string(),
        };
        on_result(&r);
        results.push(r);
    } else {
        // Start ASN lookup in background (non-blocking)
        let ip_count = ip_to_host.len();
        let r = ReconStepResult {
            name: "asn",
            success: true,
            message: format!(
                "ASN lookup started for {} IPs (running in background)",
                ip_count
            ),
        };
        on_result(&r);
        results.push(r);

        // Note: ASN results won't be available for hosting detection in this run
        // Run separately with: balebos asn-lookup
    }

    // 8. Hosting Detection
    on_step("Detecting hosting providers...");
    let mut dns_map: HashMap<String, probe::DnsObservation> = HashMap::new();
    for obs in dns_observations {
        dns_map.entry(obs.target.clone()).or_insert(obs);
    }

    let mut http_map: HashMap<String, probe::HttpObservation> = HashMap::new();
    for obs in http_observations {
        http_map.entry(obs.target.clone()).or_insert(obs);
    }

    match hosting::detect_hosting_batch(&targets, &dns_map, &http_map, &asn_by_ip, storage) {
        Ok(obs) => {
            let detected = obs.iter().filter(|o| o.provider_id.is_some()).count();
            let r = ReconStepResult {
                name: "hosting",
                success: true,
                message: format!("Hosting detected for {} targets", detected),
            };
            on_result(&r);
            results.push(r);
        }
        Err(e) => {
            let r = ReconStepResult {
                name: "hosting",
                success: false,
                message: format!("Hosting detection failed: {}", e),
            };
            on_result(&r);
            results.push(r);
        }
    }

    Ok(results)
}

/// Get all unique domains and subdomains currently in inventory
fn get_all_assets(storage: &Arc<dyn StorageEngine>) -> Result<Vec<String>> {
    use std::collections::HashSet;

    let mut seen = HashSet::new();
    let mut assets: Vec<String> = storage
        .read_all_records()?
        .items
        .into_iter()
        .filter_map(|item| {
            if seen.insert(item.name.clone()) {
                Some(item.name)
            } else {
                None
            }
        })
        .collect();

    assets.sort();
    Ok(assets)
}

/// Print summary statistics
fn print_summary(stats: &BatchStats, domains: &[String]) {
    println!("╔═══════════════════════════════════════╗");
    println!("║     📊 BATCH PROCESSING SUMMARY      ║");
    println!("╠═══════════════════════════════════════╣");
    println!("║ Input domains:        {:>22} ║", domains.len());
    println!("║ Successfully scanned: {:>22} ║", stats.domains_added);
    println!("║ Failed to scan:       {:>22} ║", stats.domains_failed);
    println!("║ Subdomains found:     {:>22} ║", stats.subdomains_found);
    println!(
        "║ DNS checks:           {:>22} ║",
        stats.dns_checks_completed
    );
    println!(
        "║ HTTP checks:          {:>22} ║",
        stats.http_checks_completed
    );
    println!("║ HTTP errors:          {:>22} ║", stats.http_errors);
    println!(
        "║ Duration:             {:>22} s ║",
        stats.total_runtime_secs
    );
    println!("╚═══════════════════════════════════════╝");
}

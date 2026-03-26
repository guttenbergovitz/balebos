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

/// Run full reconnaissance on a batch of domains
pub fn batch_process(
    filepath: &str,
    storage: &Arc<dyn StorageEngine>,
    ignore_patterns: &[String],
) -> Result<BatchStats> {
    let start = Instant::now();

    // Load domains from file
    println!("📂 Loading domains from: {}", filepath);
    let domains = load_domains_from_file(filepath)?;
    println!("✓ Loaded {} domains\n", domains.len());

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

    // Step 1: Scan all domains (Parallel)
    println!("🔍 Step 1/7: Scanning domains and RDAP info (parallel)...");

    // Process domains in parallel
    let results: Vec<_> = domains
        .par_iter()
        .enumerate()
        .map(|(idx, domain)| {
            if crate::ignore::is_ignored(domain, ignore_patterns) {
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

    for (idx, domain, item_opt, rdap_opt, err_opt, skipped) in results {
        if let Some(err) = err_opt {
            println!(
                "  ✗ [{}/{}] {} (error: {})",
                idx + 1,
                domains.len(),
                domain,
                err
            );
            stats.domains_failed += 1;
        } else if let Some(item) = item_opt {
            let status = if skipped {
                skipped_rdap_count += 1;
                "(RDAP fresh, skipped)"
            } else if rdap_opt.is_some() {
                "(RDAP updated)"
            } else {
                "(RDAP failed/no data)"
            };
            println!("  ✓ [{}/{}] {} {}", idx + 1, domains.len(), domain, status);
            stats.domains_added += 1;
            new_items.push(item);
            if let Some(rdap) = rdap_opt {
                new_rdaps.push(rdap);
            }
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

    // Step 2: Probe subdomains for all domains
    println!("{} Step 2/7: Discovering subdomains in batch...", '\u{1F577}');
    let mut valid_domains = Vec::new();
    for domain in &domains {
        if !crate::ignore::is_ignored(domain, ignore_patterns) {
            valid_domains.push(domain.clone());
        }
    }

    if valid_domains.is_empty() {
        println!("  → No domains to check");
    } else {
        print!(
            "  ⟳ Running subfinder for {} domains ... ",
            valid_domains.len()
        );
        use std::io::Write;
        let _ = std::io::stdout().flush();

        match probe::probe_subdomains_batch(&valid_domains, storage) {
            Ok(subdomains) => {
                println!("found {} subdomains", subdomains.len());
                stats.subdomains_found += subdomains.len();
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
    println!();

    // Get all current domains and subdomains
    let all_assets = get_all_assets(storage)?;

    // Step 3: Probe DNS for all domains and subdomains
    println!("📡 Step 3/7: Resolving DNS records...");

    let mut dns_targets = Vec::new();
    for asset in &all_assets {
        if !crate::ignore::is_ignored(asset, ignore_patterns) {
            dns_targets.push(asset.clone());
        }
    }

    if dns_targets.is_empty() {
        println!("  → No targets to check");
    } else {
        print!("  ⟳ Resolving {} targets in batch ", dns_targets.len());
        use std::io::Write;
        let _ = std::io::stdout().flush();

        match probe::probe_dns_batch(&dns_targets, storage) {
            Ok(observations) => {
                println!(" ✓");
                println!("  → Recorded {} DNS observations", observations.len());
                stats.dns_checks_completed += observations.len();
            }
            Err(e) => {
                println!(" ✗");
                println!("  → DNS batch probe failed: {}", e);
            }
        }
    }
    println!();

    // Step 4: Probe HTTP for all domains and subdomains
    println!("🌐 Step 4/7: Checking HTTP status...");

    let mut http_targets = Vec::new();
    for asset in &all_assets {
        if !crate::ignore::is_ignored(asset, ignore_patterns) {
            http_targets.push(asset.clone());
        }
    }

    // Store HTTP observations for later hosting detection
    let mut http_observations = Vec::new();

    if http_targets.is_empty() {
        println!("  → No targets to check");
    } else {
        print!("  {} Checking {} targets in batch ", '\u{27F3}', http_targets.len());
        use std::io::Write;
        let _ = std::io::stdout().flush();

        match probe::probe_http_batch(&http_targets, storage) {
            Ok(observations) => {
                println!(" {}", '\u{2713}');
                println!("  → Recorded {} HTTP observations", observations.len());

                // For batch stats, we estimate based on observations returned
                for obs in &observations {
                    if obs.status.is_some() {
                        stats.http_checks_completed += 1;
                    } else if obs.error_kind.is_some() {
                        stats.http_errors += 1;
                    }
                }
                http_observations = observations;
            }
            Err(e) => {
                println!(" {}", '\u{2717}');
                println!("  → HTTP batch probe failed: {}", e);
            }
        }
    }
    println!();

    // Step 5: Probe ASN for all unique IPs
    println!("🔍 Step 5/7: Looking up ASN information...");

    // Get latest DNS observations for IP extraction
    let dns_records = storage.read_all_records()?.dns_observations;

    // Build IP to host mapping
    let ip_to_host = probe::asn::build_ip_host_map(&dns_records);

    let mut asn_by_ip: HashMap<String, probe::AsnObservation> = HashMap::new();

    if ip_to_host.is_empty() {
        println!("  → No IPs to look up");
    } else {
        print!("  {} Looking up {} unique IPs ", '\u{27F3}', ip_to_host.len());
        use std::io::Write;
        let _ = std::io::stdout().flush();

        match probe::probe_asn_batch(&ip_to_host, storage) {
            Ok(observations) => {
                println!(" {}", '\u{2713}');
                println!("  → Recorded {} ASN records", observations.len());
                for obs in observations {
                    asn_by_ip.insert(obs.target.clone(), obs);
                }
            }
            Err(e) => {
                println!(" {}", '\u{2717}');
                println!("  → ASN lookup failed: {} (is asnmap installed?)", e);
            }
        }
    }
    println!();

    // Step 6: Detect hosting providers
    println!("🏠 Step 6/7: Detecting hosting providers...");

    // Build lookup maps for hosting detection
    let mut dns_map: HashMap<String, probe::DnsObservation> = HashMap::new();
    for obs in dns_records {
        dns_map.entry(obs.target.clone()).or_insert(obs);
    }

    let mut http_map: HashMap<String, probe::HttpObservation> = HashMap::new();
    for obs in http_observations {
        http_map.entry(obs.target.clone()).or_insert(obs);
    }

    if all_assets.is_empty() {
        println!("  → No targets to analyze");
    } else {
        print!("  {} Analyzing {} targets ", '\u{27F3}', all_assets.len());
        use std::io::Write;
        let _ = std::io::stdout().flush();

        match hosting::detect_hosting_batch(
            &all_assets,
            &dns_map,
            &http_map,
            &asn_by_ip,
            storage,
        ) {
            Ok(observations) => {
                println!(" {}", '\u{2713}');

                // Count detection results
                let detected_count = observations.iter().filter(|o| o.provider_id.is_some()).count();
                let cdn_count = observations.iter().filter(|o| o.cdn_detected).count();

                println!(
                    "  → Analyzed {} assets: {} providers identified, {} behind CDN",
                    observations.len(),
                    detected_count,
                    cdn_count
                );
            }
            Err(e) => {
                println!(" {}", '\u{2717}');
                println!("  → Hosting detection failed: {}", e);
            }
        }
    }
    println!();

    // Calculate stats
    stats.total_runtime_secs = start.elapsed().as_secs();

    // Print summary
    print_summary(&stats, &domains);

    Ok(stats)
}

/// Run full reconnaissance on a single domain
pub fn run_single_recon(
    domain: &str,
    storage: &Arc<dyn StorageEngine>,
    ignore_patterns: &[String],
) -> crate::error::Result<()> {
    if crate::ignore::is_ignored(domain, ignore_patterns) {
        return Ok(());
    }

    // 1. Scan/Add domain
    crate::app::scan::scan_domain(domain, storage)?;

    // 2. Run RDAP probe (apex domain)
    let _ = probe::run_rdap_probe(storage, domain);

    // 3. Discover subdomains
    let subdomains = probe::probe_subdomains(domain, storage).unwrap_or_default();

    // 4. Prepare targets for batch probes (domain + subdomains)
    let mut targets = vec![domain.to_string()];
    for sub in subdomains {
        if !crate::ignore::is_ignored(&sub, ignore_patterns) {
            targets.push(sub);
        }
    }

    // 5. Batch DNS Probe
    let dns_observations = probe::probe_dns_batch(&targets, storage).unwrap_or_default();

    // 6. Batch HTTP Probe
    let http_observations = probe::probe_http_batch(&targets, storage).unwrap_or_default();

    // 7. ASN Probe
    let ip_to_host = probe::asn::build_ip_host_map(&dns_observations);
    let mut asn_by_ip: HashMap<String, probe::AsnObservation> = HashMap::new();
    if !ip_to_host.is_empty() {
        if let Ok(asn_obs) = probe::probe_asn_batch(&ip_to_host, storage) {
            for obs in asn_obs {
                asn_by_ip.insert(obs.target.clone(), obs);
            }
        }
    }

    // 8. Hosting Detection
    let mut dns_map: HashMap<String, probe::DnsObservation> = HashMap::new();
    for obs in dns_observations {
        dns_map.entry(obs.target.clone()).or_insert(obs);
    }

    let mut http_map: HashMap<String, probe::HttpObservation> = HashMap::new();
    for obs in http_observations {
        http_map.entry(obs.target.clone()).or_insert(obs);
    }

    let _ = hosting::detect_hosting_batch(&targets, &dns_map, &http_map, &asn_by_ip, storage);

    Ok(())
}

/// Get all unique domains and subdomains currently in inventory
fn get_all_assets(storage: &Arc<dyn StorageEngine>) -> Result<Vec<String>> {
    use std::collections::HashSet;

    let mut seen = HashSet::new();
    let mut assets: Vec<String> = storage.read_all_records()?
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

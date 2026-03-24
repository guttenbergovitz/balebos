use crate::app::probe;
use crate::domain::normalize_domain;
use crate::error::Result;
use crate::storage::JsonlStorage;
use rayon::prelude::*;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
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
    storage: &JsonlStorage,
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
    let existing_records = crate::storage::read_records(storage).ok();
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
    println!("🔍 Step 1/5: Scanning domains and RDAP info (parallel)...");

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
    println!("🕷️  Step 2/5: Discovering subdomains in batch...");
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
    println!("📡 Step 3/5: Resolving DNS records...");

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
    println!("🌐 Step 4/5: Checking HTTP status...");

    let mut http_targets = Vec::new();
    for asset in &all_assets {
        if !crate::ignore::is_ignored(asset, ignore_patterns) {
            http_targets.push(asset.clone());
        }
    }

    if http_targets.is_empty() {
        println!("  → No targets to check");
    } else {
        print!("  ⟳ Checking {} targets in batch ", http_targets.len());
        use std::io::Write;
        let _ = std::io::stdout().flush();

        match probe::probe_http_batch(&http_targets, storage) {
            Ok(observations) => {
                println!(" ✓");
                println!("  → Recorded {} HTTP observations", observations.len());

                // For batch stats, we estimate based on observations returned
                for obs in observations {
                    if obs.status.is_some() {
                        stats.http_checks_completed += 1;
                    } else if obs.error_kind.is_some() {
                        stats.http_errors += 1;
                    }
                }
            }
            Err(e) => {
                println!(" ✗");
                println!("  → HTTP batch probe failed: {}", e);
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
    storage: &JsonlStorage,
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
    let _ = probe::probe_dns_batch(&targets, storage);

    // 6. Batch HTTP Probe
    let _ = probe::probe_http_batch(&targets, storage);

    Ok(())
}

/// Get all unique domains and subdomains currently in inventory
fn get_all_assets(storage: &JsonlStorage) -> Result<Vec<String>> {
    use std::collections::HashSet;

    let mut seen = HashSet::new();
    let mut assets: Vec<String> = crate::storage::read_records(storage)?
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

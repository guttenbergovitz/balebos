use crate::app::probe;
use crate::app::probe_dns;
use crate::app::probe_http;
use crate::domain::normalize_domain;
use crate::storage::JsonlStorage;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
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
fn load_domains_from_file(filepath: &str) -> Result<Vec<String>, io::Error> {
    let path = Path::new(filepath);

    if !path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("File not found: {}", filepath),
        ));
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
) -> Result<BatchStats, io::Error> {
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

    // Step 1: Scan all domains
    println!("🔍 Step 1/4: Scanning domains...");
    for (idx, domain) in domains.iter().enumerate() {
        if crate::ignore::is_ignored(domain, ignore_patterns) {
            println!("  ⊘ [{}/{}] {} (ignored)", idx + 1, domains.len(), domain);
            continue;
        }

        match crate::app::scan::scan_domain(domain, storage) {
            Ok(_) => {
                println!("  ✓ [{}/{}] {}", idx + 1, domains.len(), domain);
                stats.domains_added += 1;
            }
            Err(e) => {
                println!(
                    "  ✗ [{}/{}] {} (error: {})",
                    idx + 1,
                    domains.len(),
                    domain,
                    e
                );
                stats.domains_failed += 1;
            }
        }
    }
    println!();

    // Step 2: Probe subdomains for each domain
    println!("🕷️  Step 2/4: Discovering subdomains...");
    for (idx, domain) in domains.iter().enumerate() {
        if crate::ignore::is_ignored(domain, ignore_patterns) {
            continue;
        }

        print!("  ⟳ [{}/{}] {} ... ", idx + 1, domains.len(), domain);
        match probe::probe_subdomains(domain, storage) {
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
    println!("📡 Step 3/4: Resolving DNS records...");
    
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
        
        match probe_dns::probe_dns_batch(&dns_targets, storage) {
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
    println!("🌐 Step 4/4: Checking HTTP status...");
    
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

        match probe_http::probe_http_batch(&http_targets, storage) {
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

/// Get all unique domains and subdomains currently in inventory
fn get_all_assets(storage: &JsonlStorage) -> Result<Vec<String>, io::Error> {
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
    println!("║ DNS checks:           {:>22} ║", stats.dns_checks_completed);
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

use crate::app::probe;
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
        http_checks_completed: 0,
        http_errors: 0,
        total_runtime_secs: 0,
    };

    // Step 1: Scan all domains
    println!("🔍 Step 1/3: Scanning domains...");
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
    println!("🕷️  Step 2/3: Discovering subdomains...");
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

    // Step 3: Probe HTTP for all domains and subdomains
    println!("🌐 Step 3/3: Checking HTTP status...");

    // Get all current domains and subdomains
    let all_assets = get_all_assets(storage)?;
    let mut completed = 0;
    let total = all_assets.len();

    for asset in all_assets {
        if crate::ignore::is_ignored(&asset, ignore_patterns) {
            continue;
        }

        print!("  ⟳ [{}/{}] {} ", completed + 1, total, asset);

        match probe_http::http_check(&asset, storage) {
            Ok(obs) => {
                if let Some(status) = obs.status {
                    print!("→ {}", status);
                    if status >= 500 {
                        print!(" 🔴");
                    } else if status >= 400 {
                        print!(" 🟡");
                    } else {
                        print!(" 🟢");
                    }
                    stats.http_checks_completed += 1;
                } else if obs.error_kind.is_some() {
                    print!("→ ERROR");
                    stats.http_errors += 1;
                }
                println!(" ({}ms)", obs.response_time_ms);
            }
            Err(e) => {
                println!("✗ (error: {})", e);
            }
        }

        completed += 1;
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

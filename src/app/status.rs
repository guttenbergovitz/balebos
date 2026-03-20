/// Domain status report module for displaying HTTP probe results.
///
/// This module provides functionality to generate health reports of all assets in the
/// inventory, showing their HTTP probe status grouped by category. It helps identify
/// which domains are alive, have problems, or haven't been probed yet.
///
/// # Features
///
/// - Categorizes all assets by HTTP status (200, 3xx/4xx, 5xx, network errors)
/// - Shows statistics (count and percentage) for each category
/// - Displays latest probe result for each asset (deduplicates by most recent check)
/// - Includes response times for successful connections
/// - Shows detailed error information for failures
///
/// # Status Categories
///
/// - 🟢 **Alive (200)** - Working domains
/// - 🟡 **Client Errors (3xx/4xx)** - Redirect/auth/notfound issues
/// - 🔴 **Server Errors (5xx)** - Backend problems
/// - ⚠️ **Network Errors** - DNS, timeout, TLS, connection failures
/// - ⚪ **Unchecked** - Not yet probed
///
/// # Example Output
///
/// ```text
/// Domain Status Report
/// ====================
/// Total assets: 1001
///
/// Status Summary:
///   🟢 Alive (HTTP 200):         654 ( 65.3%)
///   🟡 Client errors (4xx):      133 ( 13.3%)
///   🔴 Server errors (5xx):       32 (  3.2%)
///   ⚠️  Network errors:           182 ( 18.2%)
/// ```
use crate::storage::JsonlStorage;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

#[derive(Deserialize)]
struct RecordHeader {
    item_type: String,
}

#[derive(Deserialize, Clone, Debug)]
struct HttpCheck {
    target: String,
    status: Option<u16>,
    response_time_ms: u32,
    checked_at: String,
    error_kind: Option<String>,
    error_message: Option<String>,
}

/// Internal structure to hold categorized status data
struct StatusReport {
    alive: Vec<HttpCheck>,
    client_errors: Vec<HttpCheck>,
    server_errors: Vec<HttpCheck>,
    network_errors: Vec<HttpCheck>,
    unchecked: Vec<HttpCheck>,
    total: usize,
}

/// Collect and categorize HTTP check results from storage
fn collect_status_report(storage: &JsonlStorage) -> Result<StatusReport, io::Error> {
    let file_path = storage.get_filepath();
    let path = Path::new(file_path);

    if !path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Inventory file not found",
        ));
    }

    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    // Collect latest HTTP check for each target (dedup by most recent)
    let mut status_map: BTreeMap<String, HttpCheck> = BTreeMap::new();

    for line_result in reader.lines() {
        let line = line_result?;

        if line.trim().is_empty() {
            continue;
        }

        let header = match serde_json::from_str::<RecordHeader>(&line) {
            Ok(h) => h,
            Err(_) => continue,
        };

        if header.item_type != "http_check" {
            continue;
        }

        if let Ok(check) = serde_json::from_str::<HttpCheck>(&line) {
            status_map.insert(check.target.clone(), check);
        }
    }

    // Categorize by status
    let mut alive = Vec::new();
    let mut client_errors = Vec::new();
    let mut server_errors = Vec::new();
    let mut network_errors = Vec::new();
    let mut unchecked = Vec::new();

    for (_, check) in status_map.iter() {
        match check.status {
            Some(200) => alive.push(check.clone()),
            Some(code) if code >= 300 && code < 400 => client_errors.push(check.clone()),
            Some(code) if code >= 400 && code < 500 => client_errors.push(check.clone()),
            Some(code) if code >= 500 && code < 600 => server_errors.push(check.clone()),
            Some(_) => unchecked.push(check.clone()),
            None => {
                if check.error_kind.is_some() {
                    network_errors.push(check.clone());
                } else {
                    unchecked.push(check.clone());
                }
            }
        }
    }

    let total = status_map.len();

    Ok(StatusReport {
        alive,
        client_errors,
        server_errors,
        network_errors,
        unchecked,
        total,
    })
}

/// Display status of all domains/subdomains with their HTTP probe results
/// Shows live/dead status with HTTP status codes and error information
pub fn display_domain_status(storage: &JsonlStorage) -> Result<(), io::Error> {
    let report = collect_status_report(storage)?;

    if report.total == 0 {
        println!("No HTTP checks found in inventory.");
        return Ok(());
    }

    let total = report.total;
    println!("Domain Status Report");
    println!("====================");
    println!("Total assets: {}\n", total);

    // Print statistics
    println!("Status Summary:");
    println!(
        "  🟢 Alive (HTTP 200):        {:4} ({:5.1}%)",
        report.alive.len(),
        (report.alive.len() as f64 / total as f64) * 100.0
    );
    println!(
        "  🟡 Client errors (4xx):     {:4} ({:5.1}%)",
        report.client_errors.len(),
        (report.client_errors.len() as f64 / total as f64) * 100.0
    );
    println!(
        "  🔴 Server errors (5xx):     {:4} ({:5.1}%)",
        report.server_errors.len(),
        (report.server_errors.len() as f64 / total as f64) * 100.0
    );
    println!(
        "  ⚠️  Network errors:          {:4} ({:5.1}%)",
        report.network_errors.len(),
        (report.network_errors.len() as f64 / total as f64) * 100.0
    );
    println!(
        "  ⚪ Unchecked:               {:4} ({:5.1}%)\n",
        report.unchecked.len(),
        (report.unchecked.len() as f64 / total as f64) * 100.0
    );

    // Print detailed listings
    if !report.alive.is_empty() {
        println!("🟢 ALIVE (HTTP 200) - {} assets", report.alive.len());
        println!("─────────────────────────────────");
        for check in report.alive.iter().take(50) {
            println!("  {} ({} ms)", check.target, check.response_time_ms);
        }
        if report.alive.len() > 50 {
            println!("  ... and {} more", report.alive.len() - 50);
        }
        println!();
    }

    if !report.server_errors.is_empty() {
        println!(
            "🔴 SERVER ERRORS (5xx) - {} assets",
            report.server_errors.len()
        );
        println!("──────────────────────────────────");
        for check in report.server_errors.iter().take(30) {
            let status_str = check.status.map(|s| s.to_string()).unwrap_or_default();
            println!("  {} - HTTP {}", check.target, status_str);
        }
        if report.server_errors.len() > 30 {
            println!("  ... and {} more", report.server_errors.len() - 30);
        }
        println!();
    }

    if !report.client_errors.is_empty() {
        println!(
            "🟡 CLIENT ERRORS (3xx/4xx) - {} assets",
            report.client_errors.len()
        );
        println!("─────────────────────────────────────");
        for check in report.client_errors.iter().take(30) {
            let status_str = check.status.map(|s| s.to_string()).unwrap_or_default();
            println!("  {} - HTTP {}", check.target, status_str);
        }
        if report.client_errors.len() > 30 {
            println!("  ... and {} more", report.client_errors.len() - 30);
        }
        println!();
    }

    if !report.network_errors.is_empty() {
        println!(
            "⚠️  NETWORK ERRORS - {} assets",
            report.network_errors.len()
        );
        println!("─────────────────────────────");
        for check in report.network_errors.iter().take(30) {
            let error_kind = check.error_kind.as_deref().unwrap_or("unknown");
            println!(
                "  {} - {} ({}ms)",
                check.target, error_kind, check.response_time_ms
            );
        }
        if report.network_errors.len() > 30 {
            println!("  ... and {} more", report.network_errors.len() - 30);
        }
        println!();
    }

    Ok(())
}

/// Export status report to CSV format
pub fn export_status_csv(storage: &JsonlStorage, output_file: &str) -> Result<(), io::Error> {
    let report = collect_status_report(storage)?;

    let mut csv_lines =
        vec!["target,status,http_code,response_time_ms,error_kind,checked_at".to_string()];

    // Collect all checks
    let mut all_checks = Vec::new();
    all_checks.extend(report.alive.iter().map(|c| (c, "200")));
    all_checks.extend(report.server_errors.iter().map(|c| (c, "5xx")));
    all_checks.extend(report.client_errors.iter().map(|c| (c, "4xx")));
    all_checks.extend(report.network_errors.iter().map(|c| (c, "error")));
    all_checks.extend(report.unchecked.iter().map(|c| (c, "unchecked")));

    for (check, status_label) in all_checks {
        let http_code = check.status.map(|s| s.to_string()).unwrap_or_else(|| {
            check
                .error_kind
                .clone()
                .unwrap_or_else(|| "N/A".to_string())
        });
        let error_kind = check.error_kind.clone().unwrap_or_else(|| "".to_string());
        csv_lines.push(format!(
            "\"{}\",\"{}\",\"{}\",{},\"{}\",\"{}\"",
            check.target,
            status_label,
            http_code,
            check.response_time_ms,
            error_kind,
            check.checked_at
        ));
    }

    std::fs::write(output_file, csv_lines.join("\n"))?;
    println!("✓ Status report exported to: {}", output_file);
    Ok(())
}

/// Export status report to Markdown format
pub fn export_status_markdown(storage: &JsonlStorage, output_file: &str) -> Result<(), io::Error> {
    let report = collect_status_report(storage)?;
    let total = report.total;

    let mut md = String::new();
    md.push_str("# Domain Status Report\n\n");
    md.push_str(&format!(
        "**Generated:** {}\n\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    md.push_str("## Summary\n\n");
    md.push_str(&format!("| Status | Count | Percentage |\n"));
    md.push_str(&format!("|--------|-------|------------|\n"));
    md.push_str(&format!(
        "| 🟢 Alive (200) | {} | {:.1}% |\n",
        report.alive.len(),
        (report.alive.len() as f64 / total as f64) * 100.0
    ));
    md.push_str(&format!(
        "| 🔴 Server Errors (5xx) | {} | {:.1}% |\n",
        report.server_errors.len(),
        (report.server_errors.len() as f64 / total as f64) * 100.0
    ));
    md.push_str(&format!(
        "| 🟡 Client Errors (4xx) | {} | {:.1}% |\n",
        report.client_errors.len(),
        (report.client_errors.len() as f64 / total as f64) * 100.0
    ));
    md.push_str(&format!(
        "| ⚠️ Network Errors | {} | {:.1}% |\n",
        report.network_errors.len(),
        (report.network_errors.len() as f64 / total as f64) * 100.0
    ));
    md.push_str(&format!(
        "| ⚪ Unchecked | {} | {:.1}% |\n\n",
        report.unchecked.len(),
        (report.unchecked.len() as f64 / total as f64) * 100.0
    ));

    // Alive section
    if !report.alive.is_empty() {
        md.push_str(&format!(
            "## 🟢 Alive (HTTP 200) - {} assets\n\n",
            report.alive.len()
        ));
        md.push_str("| Domain | Response Time (ms) |\n");
        md.push_str("|--------|-------------------|\n");
        for check in &report.alive {
            md.push_str(&format!(
                "| {} | {} |\n",
                check.target, check.response_time_ms
            ));
        }
        md.push_str("\n");
    }

    // Server errors section
    if !report.server_errors.is_empty() {
        md.push_str(&format!(
            "## 🔴 Server Errors (5xx) - {} assets\n\n",
            report.server_errors.len()
        ));
        md.push_str("| Domain | HTTP Status |\n");
        md.push_str("|--------|-------------|\n");
        for check in &report.server_errors {
            let status = check.status.map(|s| s.to_string()).unwrap_or_default();
            md.push_str(&format!("| {} | {} |\n", check.target, status));
        }
        md.push_str("\n");
    }

    // Client errors section
    if !report.client_errors.is_empty() {
        md.push_str(&format!(
            "## 🟡 Client Errors (4xx) - {} assets\n\n",
            report.client_errors.len()
        ));
        md.push_str("| Domain | HTTP Status |\n");
        md.push_str("|--------|-------------|\n");
        for check in &report.client_errors {
            let status = check.status.map(|s| s.to_string()).unwrap_or_default();
            md.push_str(&format!("| {} | {} |\n", check.target, status));
        }
        md.push_str("\n");
    }

    // Network errors section
    if !report.network_errors.is_empty() {
        md.push_str(&format!(
            "## ⚠️ Network Errors - {} assets\n\n",
            report.network_errors.len()
        ));
        md.push_str("| Domain | Error Type | Response Time (ms) |\n");
        md.push_str("|--------|------------|-------------------|\n");
        for check in &report.network_errors {
            let error = check.error_kind.as_deref().unwrap_or("unknown");
            md.push_str(&format!(
                "| {} | {} | {} |\n",
                check.target, error, check.response_time_ms
            ));
        }
        md.push_str("\n");
    }

    std::fs::write(output_file, md)?;
    println!("✓ Status report exported to: {}", output_file);
    Ok(())
}

/// Export status report to HTML format
pub fn export_status_html(storage: &JsonlStorage, output_file: &str) -> Result<(), io::Error> {
    let report = collect_status_report(storage)?;
    let total = report.total;

    let mut html = String::new();
    html.push_str("<!DOCTYPE html>\n");
    html.push_str("<html>\n");
    html.push_str("<head>\n");
    html.push_str("  <meta charset=\"UTF-8\">\n");
    html.push_str("  <title>Domain Status Report</title>\n");
    html.push_str("  <style>\n");
    html.push_str("    body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 20px; background: #f5f5f5; }\n");
    html.push_str("    .container { max-width: 1200px; margin: 0 auto; background: white; padding: 20px; border-radius: 8px; }\n");
    html.push_str("    h1 { color: #333; border-bottom: 2px solid #ddd; padding-bottom: 10px; }\n");
    html.push_str("    h2 { color: #555; margin-top: 30px; }\n");
    html.push_str("    .summary { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 15px; margin: 20px 0; }\n");
    html.push_str("    .stat { padding: 15px; border-radius: 5px; text-align: center; }\n");
    html.push_str("    .stat.alive { background: #d4edda; border-left: 4px solid #28a745; }\n");
    html.push_str(
        "    .stat.server-error { background: #f8d7da; border-left: 4px solid #dc3545; }\n",
    );
    html.push_str(
        "    .stat.client-error { background: #fff3cd; border-left: 4px solid #ffc107; }\n",
    );
    html.push_str(
        "    .stat.network-error { background: #d1ecf1; border-left: 4px solid #17a2b8; }\n",
    );
    html.push_str("    .stat-count { font-size: 24px; font-weight: bold; }\n");
    html.push_str("    .stat-label { font-size: 14px; color: #666; }\n");
    html.push_str("    table { width: 100%; border-collapse: collapse; margin: 15px 0; }\n");
    html.push_str("    th { background: #f0f0f0; padding: 10px; text-align: left; border-bottom: 2px solid #ddd; }\n");
    html.push_str("    td { padding: 8px 10px; border-bottom: 1px solid #eee; }\n");
    html.push_str("    tr:hover { background: #f9f9f9; }\n");
    html.push_str("  </style>\n");
    html.push_str("</head>\n");
    html.push_str("<body>\n");
    html.push_str("  <div class=\"container\">\n");
    html.push_str("    <h1>Domain Status Report</h1>\n");
    html.push_str(&format!(
        "    <p><strong>Generated:</strong> {}</p>\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    html.push_str(&format!(
        "    <p><strong>Total Assets:</strong> {}</p>\n",
        total
    ));

    // Summary stats
    html.push_str("    <div class=\"summary\">\n");
    html.push_str(&format!(
        "      <div class=\"stat alive\">\n        <div class=\"stat-count\">{}</div>\n        <div class=\"stat-label\">🟢 Alive (200)</div>\n        <div class=\"stat-label\">{:.1}%</div>\n      </div>\n",
        report.alive.len(),
        (report.alive.len() as f64 / total as f64) * 100.0
    ));
    html.push_str(&format!(
        "      <div class=\"stat server-error\">\n        <div class=\"stat-count\">{}</div>\n        <div class=\"stat-label\">🔴 Server Errors</div>\n        <div class=\"stat-label\">{:.1}%</div>\n      </div>\n",
        report.server_errors.len(),
        (report.server_errors.len() as f64 / total as f64) * 100.0
    ));
    html.push_str(&format!(
        "      <div class=\"stat client-error\">\n        <div class=\"stat-count\">{}</div>\n        <div class=\"stat-label\">🟡 Client Errors</div>\n        <div class=\"stat-label\">{:.1}%</div>\n      </div>\n",
        report.client_errors.len(),
        (report.client_errors.len() as f64 / total as f64) * 100.0
    ));
    html.push_str(&format!(
        "      <div class=\"stat network-error\">\n        <div class=\"stat-count\">{}</div>\n        <div class=\"stat-label\">⚠️ Network Errors</div>\n        <div class=\"stat-label\">{:.1}%</div>\n      </div>\n",
        report.network_errors.len(),
        (report.network_errors.len() as f64 / total as f64) * 100.0
    ));
    html.push_str("    </div>\n");

    // Tables for each section
    if !report.alive.is_empty() {
        html.push_str(&format!(
            "    <h2>🟢 Alive (HTTP 200) - {} assets</h2>\n",
            report.alive.len()
        ));
        html.push_str("    <table>\n");
        html.push_str("      <tr><th>Domain</th><th>Response Time (ms)</th></tr>\n");
        for check in &report.alive {
            html.push_str(&format!(
                "      <tr><td>{}</td><td>{}</td></tr>\n",
                check.target, check.response_time_ms
            ));
        }
        html.push_str("    </table>\n");
    }

    if !report.server_errors.is_empty() {
        html.push_str(&format!(
            "    <h2>🔴 Server Errors (5xx) - {} assets</h2>\n",
            report.server_errors.len()
        ));
        html.push_str("    <table>\n");
        html.push_str("      <tr><th>Domain</th><th>HTTP Status</th></tr>\n");
        for check in &report.server_errors {
            let status = check.status.map(|s| s.to_string()).unwrap_or_default();
            html.push_str(&format!(
                "      <tr><td>{}</td><td>{}</td></tr>\n",
                check.target, status
            ));
        }
        html.push_str("    </table>\n");
    }

    if !report.client_errors.is_empty() {
        html.push_str(&format!(
            "    <h2>🟡 Client Errors (4xx) - {} assets</h2>\n",
            report.client_errors.len()
        ));
        html.push_str("    <table>\n");
        html.push_str("      <tr><th>Domain</th><th>HTTP Status</th></tr>\n");
        for check in &report.client_errors {
            let status = check.status.map(|s| s.to_string()).unwrap_or_default();
            html.push_str(&format!(
                "      <tr><td>{}</td><td>{}</td></tr>\n",
                check.target, status
            ));
        }
        html.push_str("    </table>\n");
    }

    if !report.network_errors.is_empty() {
        html.push_str(&format!(
            "    <h2>⚠️ Network Errors - {} assets</h2>\n",
            report.network_errors.len()
        ));
        html.push_str("    <table>\n");
        html.push_str(
            "      <tr><th>Domain</th><th>Error Type</th><th>Response Time (ms)</th></tr>\n",
        );
        for check in &report.network_errors {
            let error = check.error_kind.as_deref().unwrap_or("unknown");
            html.push_str(&format!(
                "      <tr><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                check.target, error, check.response_time_ms
            ));
        }
        html.push_str("    </table>\n");
    }

    html.push_str("  </div>\n");
    html.push_str("</body>\n");
    html.push_str("</html>\n");

    std::fs::write(output_file, html)?;
    println!("✓ Status report exported to: {}", output_file);
    Ok(())
}

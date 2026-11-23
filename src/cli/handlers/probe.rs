use std::process;
use std::sync::Arc;

use crate::app::probe::HttpErrorKind;
use crate::app::{http_check, probe_subdomains};
use crate::cli::ProbeKind;
use crate::exit_codes::*;
use crate::storage::StorageEngine;

#[allow(dead_code)]
pub fn cmd_probe(
    kind: ProbeKind,
    target: &str,
    storage: &Arc<dyn StorageEngine>,
    allow_internal: bool,
) {
    let allow_internal = allow_internal || crate::app::probe::scope::internal_allowed_from_env();
    if let Err(denial) = crate::app::probe::scope::check_scope(target, allow_internal) {
        eprintln!("Refusing to probe '{target}': {denial}");
        process::exit(EXIT_INVALID_ARGUMENT);
    }

    match kind {
        ProbeKind::Subdomains => match probe_subdomains(target, storage) {
            Ok(subdomains) => {
                if subdomains.is_empty() {
                    println!("No subdomains found for {}", target);
                } else {
                    println!("Found {} subdomains for {}:", subdomains.len(), target);
                    for subdomain in &subdomains {
                        println!("- {}", subdomain);
                    }
                    println!("\nAll subdomains added to inventory.");
                    println!(
                        "Run 'balebos diff' to see changes or 'balebos check' to detect anomalies."
                    );
                }
                let _ = storage.flush();
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error probing subdomains: {}", e);
                eprintln!(
                    "\nMake sure subfinder is installed: https://github.com/projectdiscovery/subfinder"
                );
                process::exit(EXIT_EXTERNAL_TOOL_ERROR);
            }
        },
        ProbeKind::Http => match http_check(target, storage) {
            Ok(observation) => {
                println!("HTTP check completed for {}", target);

                if let Some(status) = observation.status {
                    println!("Status: {}", status);
                } else if let Some(error_kind) = observation.error_kind {
                    println!("Probe failed: {:?}", error_kind);
                    println!(
                        "Error: {}",
                        observation.error_message.as_deref().unwrap_or("unknown")
                    );
                }

                if !observation.redirect_chain.is_empty() {
                    println!(
                        "\nRedirect chain ({} hops):",
                        observation.redirect_chain.len()
                    );
                    for (i, hop) in observation.redirect_chain.iter().enumerate() {
                        println!(
                            "  {}. {} ({}) -> {}",
                            i + 1,
                            hop.url,
                            hop.status,
                            hop.redirect_to
                        );
                    }
                    if let Some(final_url) = &observation.final_url {
                        println!("  Final URL: {}", final_url);
                    }
                }

                println!("\nDetails:");
                if let Some(title) = &observation.title {
                    println!("  Title: {}", title);
                }
                if let Some(server) = &observation.webserver {
                    println!("  Server: {}", server);
                }
                if !observation.tech.is_empty() {
                    println!("  Tech: {}", observation.tech.join(", "));
                }

                println!("Response time: {}ms", observation.response_time_ms);

                println!("\nObservation added to inventory.");
                println!(
                    "Run 'balebos check' to detect anomalies or 'balebos report' for a status summary."
                );

                let _ = storage.flush();
                if let Some(error_kind) = observation.error_kind {
                    match error_kind {
                        HttpErrorKind::DnsNotFound => process::exit(EXIT_DOMAIN_NOT_FOUND),
                        _ => process::exit(EXIT_PROBE_ERROR),
                    }
                } else if let Some(status) = observation.status {
                    if status >= 500 {
                        process::exit(EXIT_HTTP_SERVER_ERROR);
                    } else if status >= 400 {
                        process::exit(EXIT_HTTP_CLIENT_ERROR);
                    }
                }
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error checking HTTP status: {}", e);
                process::exit(EXIT_HTTP_ERROR);
            }
        },
        ProbeKind::Dns => match crate::app::probe::probe_dns(target, storage) {
            Ok(obs) => {
                println!("DNS check completed for {}", obs.target);

                if !obs.a.is_empty() {
                    println!("A: {}", obs.a.join(", "));
                }
                if !obs.aaaa.is_empty() {
                    println!("AAAA: {}", obs.aaaa.join(", "));
                }
                if !obs.cname.is_empty() {
                    println!("CNAME: {}", obs.cname.join(", "));
                }

                if obs.a.is_empty() && obs.aaaa.is_empty() && obs.cname.is_empty() {
                    println!("No DNS records found.");
                }

                println!("\nObservation added to inventory.");
                let _ = storage.flush();
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error checking DNS records: {}", e);
                process::exit(EXIT_EXTERNAL_TOOL_ERROR);
            }
        },
        ProbeKind::Asn => {
            // First run DNS probe to get IPs
            match crate::app::probe::probe_dns(target, storage) {
                Ok(dns_obs) => {
                    if dns_obs.a.is_empty() && dns_obs.aaaa.is_empty() {
                        eprintln!("No IP addresses found for {}. Run DNS probe first.", target);
                        process::exit(EXIT_DOMAIN_NOT_FOUND);
                    }

                    // Build IP to host map
                    let ip_map =
                        crate::app::probe::asn::build_ip_host_map(std::slice::from_ref(&dns_obs));

                    match crate::app::probe::probe_asn_batch(&ip_map, storage) {
                        Ok(observations) => {
                            println!("ASN lookup completed for {}", target);

                            for obs in &observations {
                                println!(
                                    "  {} -> AS{} {} ({})",
                                    obs.target, obs.asn, obs.org, obs.country
                                );
                            }

                            if observations.is_empty() {
                                println!("No ASN information found.");
                            } else {
                                println!(
                                    "\n{} ASN records added to inventory.",
                                    observations.len()
                                );
                            }

                            let _ = storage.flush();
                            process::exit(EXIT_SUCCESS);
                        }
                        Err(e) => {
                            eprintln!("Error looking up ASN: {}", e);
                            eprintln!("\nMake sure asnmap is installed: https://github.com/projectdiscovery/asnmap");
                            process::exit(EXIT_EXTERNAL_TOOL_ERROR);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error resolving DNS: {}", e);
                    process::exit(EXIT_EXTERNAL_TOOL_ERROR);
                }
            }
        }
        ProbeKind::Hosting => {
            use std::collections::HashMap;

            // Run DNS probe first
            let dns_obs = match crate::app::probe::probe_dns(target, storage) {
                Ok(obs) => Some(obs),
                Err(e) => {
                    eprintln!("Warning: DNS probe failed: {}", e);
                    None
                }
            };

            // Run HTTP probe
            let http_obs = match crate::app::probe::http_check(target, storage) {
                Ok(obs) => Some(obs),
                Err(e) => {
                    eprintln!("Warning: HTTP probe failed: {}", e);
                    None
                }
            };

            // Run ASN probe if we have IPs
            let mut asn_by_ip: HashMap<String, crate::app::probe::AsnObservation> = HashMap::new();
            if let Some(ref dns) = dns_obs {
                let ip_map = crate::app::probe::asn::build_ip_host_map(std::slice::from_ref(dns));
                if !ip_map.is_empty() {
                    if let Ok(asn_obs) = crate::app::probe::probe_asn_batch(&ip_map, storage) {
                        for obs in asn_obs {
                            asn_by_ip.insert(obs.target.clone(), obs);
                        }
                    }
                }
            }

            // Run hosting detection
            match crate::hosting::detect_hosting(
                target,
                dns_obs.as_ref(),
                http_obs.as_ref(),
                &asn_by_ip,
                storage,
            ) {
                Ok(obs) => {
                    let detector = crate::hosting::HostingDetector::new();
                    let attr = detector.attribute(
                        &crate::hosting::DetectionInput {
                            target,
                            dns: dns_obs.as_ref(),
                            http: http_obs.as_ref(),
                            asn_by_ip: &asn_by_ip,
                        },
                        None,
                    );

                    println!(
                        "Infrastructure attribution for {} (rules {})",
                        target, attr.rule_version
                    );
                    if !attr.authoritative_dns.is_empty() {
                        println!("\nAuthoritative DNS:");
                        for d in &attr.authoritative_dns {
                            println!("  - {} [{}]", d.provider_name, d.status);
                        }
                    }
                    if !attr.cdn.is_empty() {
                        println!("\nCDN / Edge:");
                        for c in &attr.cdn {
                            println!("  - {} [{}]", c.provider_name, c.status);
                        }
                    }
                    if !attr.network.is_empty() {
                        println!("\nNetwork:");
                        for n in &attr.network {
                            println!("  - {} AS{} ({})", n.ip, n.asn, n.org);
                        }
                    }
                    if !attr.application_platform.is_empty() {
                        println!("\nApplication platform:");
                        for p in &attr.application_platform {
                            println!("  - {} [{}]", p.provider_name, p.status);
                        }
                    }
                    println!("\nOrigin:");
                    match &attr.origin {
                        crate::hosting::OriginAttribution::Heuristic {
                            provider_name,
                            provider_category,
                            status,
                            ..
                        } => println!(
                            "  {} ({}) [{} — heuristic]",
                            provider_name, provider_category, status
                        ),
                        crate::hosting::OriginAttribution::Conflicting { candidates, .. } => {
                            println!("  Conflicting: {}", candidates.join(", "));
                        }
                        crate::hosting::OriginAttribution::Unknown { reason, .. } => {
                            println!("  Unknown ({})", reason);
                        }
                    }

                    if !obs.evidence.is_empty() {
                        println!("\nEvidence:");
                        for ev in &obs.evidence {
                            println!("  - {}", ev);
                        }
                    }

                    println!("\nHosting observation added to inventory.");
                    let _ = storage.flush();
                    process::exit(EXIT_SUCCESS);
                }
                Err(e) => {
                    eprintln!("Error detecting hosting: {}", e);
                    process::exit(EXIT_GENERAL_ERROR);
                }
            }
        }
    }
}

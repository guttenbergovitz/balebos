use std::collections::{HashMap, HashSet};
use std::process;
use std::sync::Arc;

use crate::app::risk::{calculate_all_risks, risk_label, RiskWeights};
use crate::cli::RiskCommand;
use crate::exit_codes::*;
use crate::storage::StorageEngine;
use chrono::Utc;

pub(crate) fn cmd_risk(risk_cmd: RiskCommand, storage: &Arc<dyn StorageEngine>) {
    let records = match storage.read_all_records() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error reading inventory: {}", e);
            process::exit(EXIT_STORAGE_ERROR);
        }
    };

    // Build asset list from inventory items and all observations
    let mut asset_names: HashSet<String> = records
        .items
        .iter()
        .map(|i| i.name.clone())
        .collect();
    for obs in &records.http_observations {
        asset_names.insert(obs.target.clone());
    }
    for obs in &records.dns_observations {
        asset_names.insert(obs.target.clone());
    }
    let asset_names: Vec<String> = asset_names.into_iter().collect();

    // Build owner map from asset assignments
    let assigned_assets: HashSet<String> = records
        .asset_assignments
        .iter()
        .map(|a| a.host.clone())
        .collect();
    let mut asset_to_owner: HashMap<String, bool> = HashMap::new();
    for name in &asset_names {
        asset_to_owner.insert(name.clone(), assigned_assets.contains(name));
    }

    // Build staleness map from latest HTTP observation
    let mut asset_to_staleness: HashMap<String, u64> = HashMap::new();
    let now = Utc::now();
    for name in &asset_names {
        let latest_check = records
            .http_observations
            .iter()
            .filter(|o| o.target == *name)
            .map(|o| o.checked_at)
            .max();
        if let Some(latest) = latest_check {
            let days = now.signed_duration_since(latest).num_days().max(0) as u64;
            asset_to_staleness.insert(name.clone(), days);
        }
    }

    let weights = RiskWeights::default();

    let scores = calculate_all_risks(
        &records.findings,
        &records.finding_events,
        &records.lifecycle_events,
        &asset_names,
        &asset_to_owner,
        &asset_to_staleness,
        Some(weights),
    );

    match risk_cmd {
        RiskCommand::Report => {
            println!("Risk Report");
            println!("===========");
            println!();
            if scores.is_empty() {
                println!("No assets found in inventory.");
                process::exit(EXIT_SUCCESS);
            }
            println!("Total assets assessed: {}", scores.len());
            let avg_score = scores.iter().map(|s| s.total_score as u64).sum::<u64>() / scores.len() as u64;
            println!("Average risk score: {}", avg_score);
            println!();
            for score in &scores {
                println!(
                    "{:>3} | {:<12} | {}",
                    score.total_score,
                    risk_label(score.total_score),
                    score.asset_name
                );
            }
            process::exit(EXIT_SUCCESS);
        }
        RiskCommand::Top { n } => {
            println!("Top {} Highest Risk Assets", n);
            println!("========================={}", "=".repeat(n.to_string().len()));
            println!();
            let top: Vec<_> = scores.into_iter().take(n).collect();
            if top.is_empty() {
                println!("No assets found.");
                process::exit(EXIT_SUCCESS);
            }
            for (i, score) in top.iter().enumerate() {
                println!(
                    "{}. {} - {} ({}",
                    i + 1,
                    score.asset_name,
                    score.total_score,
                    risk_label(score.total_score)
                );
            }
            process::exit(EXIT_SUCCESS);
        }
        RiskCommand::Show { asset_name } => {
            let score = scores.into_iter().find(|s| s.asset_name == asset_name);
            match score {
                Some(s) => {
                    println!("Risk Score for '{}'", asset_name);
                    println!("{}", "=".repeat(20 + asset_name.len()));
                    println!("Total Score: {} ({})", s.total_score, risk_label(s.total_score));
                    println!("Calculated at: {}", s.calculated_at.format("%Y-%m-%d %H:%M:%S"));
                    println!();
                    if !s.severity_breakdown.is_empty() {
                        println!("Severity Breakdown:");
                        for (sev, val) in &s.severity_breakdown {
                            println!("  - {}: {}", sev, val);
                        }
                        println!();
                    }
                    if !s.category_breakdown.is_empty() {
                        println!("Category Breakdown:");
                        for (cat, val) in &s.category_breakdown {
                            println!("  - {}: {}", cat, val);
                        }
                        println!();
                    }
                    if !s.factors.is_empty() {
                        println!("Risk Factors ({}):", s.factors.len());
                        for factor in &s.factors {
                            println!("  - {} (contribution: +{})", factor.description, factor.contribution);
                        }
                    } else {
                        println!("No risk factors detected.");
                    }
                    process::exit(EXIT_SUCCESS);
                }
                None => {
                    eprintln!("Asset '{}' not found in inventory.", asset_name);
                    process::exit(EXIT_NOT_FOUND);
                }
            }
        }
    }
}

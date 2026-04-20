use std::process;
use std::sync::Arc;

use crate::app::{
    display_lifecycle_info, display_lifecycle_summary, get_lifecycle_history, set_lifecycle,
    tags::{add_tag, get_junk_assets, JUNK_TAG},
};
use crate::cli::router::AssetCommand;
use crate::exit_codes::*;
use crate::inventory::AssetLifecycle;
use crate::storage::StorageEngine;

#[allow(dead_code)]
pub(crate) fn cmd_asset(asset_cmd: AssetCommand, storage: &Arc<dyn StorageEngine>) {
    match asset_cmd {
        AssetCommand::LifecycleSet {
            asset_name,
            state,
            reason,
        } => {
            let lifecycle_state = match AssetLifecycle::from_str(&state) {
                Some(s) => s,
                None => {
                    eprintln!("Invalid lifecycle state: {}", state);
                    eprintln!(
                        "Valid states: active, under_review, deprecated, decommissioned, abandoned"
                    );
                    process::exit(EXIT_INVALID_ARGUMENT);
                }
            };

            match set_lifecycle(
                storage,
                &asset_name,
                lifecycle_state,
                reason.as_deref(),
                "cli",
            ) {
                Ok(event) => {
                    let from_str = event
                        .from_state
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "(none)".to_string());
                    println!("Lifecycle updated: {} → {}", from_str, event.to_state);
                    println!("Asset: {}", asset_name);
                    if let Some(ref r) = reason {
                        println!("Reason: {}", r);
                    }
                    process::exit(EXIT_SUCCESS);
                }
                Err(e) => {
                    eprintln!("Error setting lifecycle: {}", e);
                    process::exit(EXIT_STORAGE_ERROR);
                }
            }
        }
        AssetCommand::LifecycleShow { asset_name } => {
            match display_lifecycle_info(storage, &asset_name) {
                Ok(()) => process::exit(EXIT_SUCCESS),
                Err(e) => {
                    eprintln!("Error displaying lifecycle: {}", e);
                    process::exit(EXIT_STORAGE_ERROR);
                }
            }
        }
        AssetCommand::LifecycleHistory { asset_name } => {
            match get_lifecycle_history(storage, &asset_name) {
                Ok(history) => {
                    if history.is_empty() {
                        println!("No lifecycle history for '{}'", asset_name);
                    } else {
                        println!("Lifecycle history for '{}':", asset_name);
                        println!("{}", "-".repeat(60));
                        for event in history {
                            let from_str = event
                                .from_state
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| "(initial)".to_string());
                            println!(
                                "[{}] {} → {}",
                                event.created_at.format("%Y-%m-%d %H:%M"),
                                from_str,
                                event.to_state
                            );
                            println!("  By: {}", event.changed_by);
                            if let Some(ref reason) = event.reason {
                                println!("  Reason: {}", reason);
                            }
                        }
                    }
                    process::exit(EXIT_SUCCESS);
                }
                Err(e) => {
                    eprintln!("Error getting lifecycle history: {}", e);
                    process::exit(EXIT_STORAGE_ERROR);
                }
            }
        }
        AssetCommand::LifecycleList => match display_lifecycle_summary(storage) {
            Ok(()) => process::exit(EXIT_SUCCESS),
            Err(e) => {
                eprintln!("Error displaying lifecycle summary: {}", e);
                process::exit(EXIT_STORAGE_ERROR);
            }
        },
        AssetCommand::Junk { asset_name } => match add_tag(storage, &asset_name, JUNK_TAG) {
            Ok(()) => {
                println!("🗑  Marked '{}' as junk", asset_name);
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error marking asset as junk: {}", e);
                process::exit(EXIT_STORAGE_ERROR);
            }
        },
        AssetCommand::Unjunk { asset_name } => {
            match crate::app::tags::remove_tag(storage, &asset_name, JUNK_TAG) {
                Ok(()) => {
                    println!("✓  Unmarked '{}' as junk", asset_name);
                    process::exit(EXIT_SUCCESS);
                }
                Err(e) => {
                    eprintln!("Error unmarking asset as junk: {}", e);
                    process::exit(EXIT_STORAGE_ERROR);
                }
            }
        }
        AssetCommand::ListJunk => {
            let records = match storage.read_all_records() {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Error reading records: {}", e);
                    process::exit(EXIT_STORAGE_ERROR);
                }
            };
            let junk = get_junk_assets(&records.asset_tags);
            if junk.is_empty() {
                println!("No junk assets found.");
            } else {
                println!("Junk assets ({}):", junk.len());
                for asset in &junk {
                    println!("  🗑  {}", asset);
                }
            }
            process::exit(EXIT_SUCCESS);
        }
    }
}

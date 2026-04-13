//! Asset management commands
//!
//! Provides functionality for managing asset lifecycle states and metadata.

use std::sync::Arc;

use crate::error::Result;
use crate::inventory::{AssetLifecycle, LifecycleEvent};
use crate::storage::{StorageEngine, StorageEngineExt};

/// Set the lifecycle state for an asset
pub fn set_lifecycle(
    storage: &Arc<dyn StorageEngine>,
    asset_name: &str,
    new_state: AssetLifecycle,
    reason: Option<&str>,
    changed_by: &str,
) -> Result<LifecycleEvent> {
    // Read current state
    let records = storage.read_all_records()?;
    let current_state = crate::inventory::get_current_lifecycle(&records.lifecycle_events, asset_name);

    // Create transition event
    let event = LifecycleEvent::new(
        asset_name.to_string(),
        current_state,
        new_state,
        reason.map(|s| s.to_string()),
        changed_by.to_string(),
    );

    // Persist event
    storage.save(&event)?;

    Ok(event)
}

/// Get the current lifecycle state for an asset
pub fn get_lifecycle(
    storage: &Arc<dyn StorageEngine>,
    asset_name: &str,
) -> Result<Option<AssetLifecycle>> {
    let records = storage.read_all_records()?;
    Ok(crate::inventory::get_current_lifecycle(
        &records.lifecycle_events,
        asset_name,
    ))
}

/// Get full lifecycle history for an asset
pub fn get_lifecycle_history(
    storage: &Arc<dyn StorageEngine>,
    asset_name: &str,
) -> Result<Vec<LifecycleEvent>> {
    let records = storage.read_all_records()?;
    Ok(crate::inventory::get_lifecycle_history(
        &records.lifecycle_events,
        asset_name,
    ))
}

/// List all assets with their current lifecycle states
pub fn list_assets_with_lifecycle(
    storage: &Arc<dyn StorageEngine>,
) -> Result<Vec<(String, Option<AssetLifecycle>)>> {
    let records = storage.read_all_records()?;

    // Get unique asset names from items
    let asset_names: std::collections::HashSet<String> = records
        .items
        .iter()
        .map(|item| item.name.clone())
        .collect();

    // Get lifecycle for each asset
    let mut result: Vec<(String, Option<AssetLifecycle>)> = asset_names
        .into_iter()
        .map(|name| {
            let state = crate::inventory::get_current_lifecycle(&records.lifecycle_events, &name);
            (name, state)
        })
        .collect();

    // Sort by name
    result.sort_by(|a, b| a.0.cmp(&b.0));

    Ok(result)
}

/// Filter assets by lifecycle state
pub fn filter_by_lifecycle(
    storage: &Arc<dyn StorageEngine>,
    state: AssetLifecycle,
) -> Result<Vec<String>> {
    let records = storage.read_all_records()?;

    let assets: Vec<String> = records
        .items
        .iter()
        .map(|item| item.name.clone())
        .filter(|name| {
            let current = crate::inventory::get_current_lifecycle(&records.lifecycle_events, name);
            current == Some(state)
        })
        .collect();

    Ok(assets)
}

/// Display lifecycle information for an asset (CLI output)
pub fn display_lifecycle_info(
    storage: &Arc<dyn StorageEngine>,
    asset_name: &str,
) -> Result<()> {
    let current = get_lifecycle(storage, asset_name)?;
    let history = get_lifecycle_history(storage, asset_name)?;

    println!("Asset: {}", asset_name);
    println!();

    // Current state
    match current {
        Some(state) => {
            println!("Current State: {} {}", state.icon(), state);
        }
        None => {
            println!("Current State: Not set (implicitly Active)");
        }
    }
    println!();

    // History
    if history.is_empty() {
        println!("No lifecycle history recorded.");
    } else {
        println!("Lifecycle History:");
        println!("{}", "-".repeat(60));
        for event in history {
            let from_str = event
                .from_state
                .map(|s| s.to_string())
                .unwrap_or_else(|| "(initial)".to_string());
            println!(
                "  [{}] {} → {}",
                event.created_at.format("%Y-%m-%d %H:%M"),
                from_str,
                event.to_state
            );
            println!("    By: {}", event.changed_by);
            if let Some(ref reason) = event.reason {
                println!("    Reason: {}", reason);
            }
        }
    }

    Ok(())
}

/// Display lifecycle summary for all assets (CLI output)
pub fn display_lifecycle_summary(storage: &Arc<dyn StorageEngine>) -> Result<()> {
    let assets = list_assets_with_lifecycle(storage)?;

    if assets.is_empty() {
        println!("No assets in inventory.");
        return Ok(());
    }

    println!("Asset Lifecycle Summary");
    println!("{}", "=".repeat(60));
    println!();

    // Group by state
    let mut by_state: std::collections::HashMap<AssetLifecycle, Vec<String>> =
        std::collections::HashMap::new();
    let mut no_state: Vec<String> = Vec::new();

    for (name, state) in assets {
        match state {
            Some(s) => {
                by_state.entry(s).or_default().push(name);
            }
            None => {
                no_state.push(name);
            }
        }
    }

    // Display by state
    for (state, names) in [
        (AssetLifecycle::Active, by_state.get(&AssetLifecycle::Active)),
        (
            AssetLifecycle::UnderReview,
            by_state.get(&AssetLifecycle::UnderReview),
        ),
        (AssetLifecycle::Deprecated, by_state.get(&AssetLifecycle::Deprecated)),
        (
            AssetLifecycle::Decommissioned,
            by_state.get(&AssetLifecycle::Decommissioned),
        ),
        (AssetLifecycle::Abandoned, by_state.get(&AssetLifecycle::Abandoned)),
    ] {
        if let Some(names) = names {
            if !names.is_empty() {
                println!("{} {} ({})", state.icon(), state, names.len());
                for name in names.iter().take(10) {
                    println!("  - {}", name);
                }
                if names.len() > 10 {
                    println!("  ... and {} more", names.len() - 10);
                }
                println!();
            }
        }
    }

    // Assets with no explicit state
    if !no_state.is_empty() {
        println!("Not set (implicitly Active) ({})" , no_state.len());
        for name in no_state.iter().take(10) {
            println!("  - {}", name);
        }
        if no_state.len() > 10 {
            println!("  ... and {} more", no_state.len() - 10);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::JsonlStorage;
    use std::path::Path;

    fn create_test_storage() -> Arc<dyn StorageEngine> {
        let temp_file = format!("/tmp/test_balebos_{}.jsonl", uuid::Uuid::new_v4());
        Arc::new(JsonlStorage::new(&temp_file))
    }

    #[test]
    fn test_set_and_get_lifecycle() {
        let storage = create_test_storage();

        // Initially no lifecycle set
        let state = get_lifecycle(&storage, "example.com").unwrap();
        assert_eq!(state, None);

        // Set to Active
        set_lifecycle(
            &storage,
            "example.com",
            AssetLifecycle::Active,
            None,
            "test",
        )
        .unwrap();

        let state = get_lifecycle(&storage, "example.com").unwrap();
        assert_eq!(state, Some(AssetLifecycle::Active));

        // Transition to Deprecated
        set_lifecycle(
            &storage,
            "example.com",
            AssetLifecycle::Deprecated,
            Some("Scheduled for removal"),
            "test",
        )
        .unwrap();

        let state = get_lifecycle(&storage, "example.com").unwrap();
        assert_eq!(state, Some(AssetLifecycle::Deprecated));
    }

    #[test]
    fn test_get_lifecycle_history() {
        let storage = create_test_storage();

        // Create some history
        set_lifecycle(&storage, "example.com", AssetLifecycle::Active, None, "test").unwrap();
        set_lifecycle(
            &storage,
            "example.com",
            AssetLifecycle::UnderReview,
            Some("Evaluating"),
            "test",
        )
        .unwrap();
        set_lifecycle(
            &storage,
            "example.com",
            AssetLifecycle::Deprecated,
            Some("Decision made"),
            "test",
        )
        .unwrap();

        let history = get_lifecycle_history(&storage, "example.com").unwrap();
        assert_eq!(history.len(), 3);

        // Should be sorted by time
        assert_eq!(history[0].to_state, AssetLifecycle::Active);
        assert_eq!(history[1].to_state, AssetLifecycle::UnderReview);
        assert_eq!(history[2].to_state, AssetLifecycle::Deprecated);
    }
}

use crate::error::Result;
use crate::inventory::AssetTag;
use crate::storage::{StorageEngine, StorageEngineExt};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub fn add_tag(storage: &Arc<dyn StorageEngine>, asset_name: &str, tag: &str) -> Result<()> {
    let asset_tag = AssetTag::new(asset_name.to_string(), tag.to_string());
    storage.save(&asset_tag)?;
    // Don't println in TUI mode - caller will set status
    Ok(())
}

pub fn remove_tag(storage: &Arc<dyn StorageEngine>, asset_name: &str, tag: &str) -> Result<()> {
    let records = storage.read_all_records()?;
    let active_tags = get_active_tags(&records.asset_tags);
    
    // Check if tag exists
    let has_tag = active_tags
        .get(asset_name)
        .map(|tags| tags.contains(tag))
        .unwrap_or(false);

    if !has_tag {
        // Silently skip if tag doesn't exist - TUI will handle status
        return Ok(());
    }

    // Create removal record
    let removal = AssetTag::removed(asset_name.to_string(), tag.to_string());
    storage.save(&removal)?;
    Ok(())
}

pub fn list_tags(storage: &Arc<dyn StorageEngine>, asset_name: Option<&str>) -> Result<()> {
    let records = storage.read_all_records()?;
    let active_tags = get_active_tags(&records.asset_tags);

    if let Some(name) = asset_name {
        let tags: Vec<_> = active_tags
            .get(name)
            .map(|tags| {
                let mut sorted: Vec<_> = tags.iter().collect();
                sorted.sort();
                sorted
            })
            .unwrap_or_default();

        if tags.is_empty() {
            println!("No tags for asset: {}", name);
        } else {
            println!("Tags for '{}':", name);
            for tag in tags {
                println!("  • {}", tag);
            }
        }
    } else {
        if active_tags.is_empty() {
            println!("No tags found");
            return Ok(());
        }

        println!("All asset tags:");
        println!("{}", "=".repeat(50));
        
        for (asset, tags) in &active_tags {
            let mut sorted_tags: Vec<_> = tags.iter().collect();
            sorted_tags.sort();
            println!("\n{}:", asset);
            for tag in sorted_tags {
                println!("  • {}", tag);
            }
        }
        
        let all_unique_tags: BTreeSet<_> = active_tags
            .values()
            .flat_map(|tags| tags.iter())
            .collect();
        
        println!("\n{}", "=".repeat(50));
        println!("Total unique tags: {}", all_unique_tags.len());
        println!("Tagged assets: {}", active_tags.len());
    }

    Ok(())
}

pub fn list_all_tag_names(storage: &Arc<dyn StorageEngine>) -> Result<()> {
    let records = storage.read_all_records()?;
    let active_tags = get_active_tags(&records.asset_tags);
    
    let all_unique_tags: BTreeSet<_> = active_tags
        .values()
        .flat_map(|tags| tags.iter())
        .collect();

    if all_unique_tags.is_empty() {
        println!("No tags defined");
    } else {
        println!("Available tags:");
        for tag in all_unique_tags {
            println!("  • {}", tag);
        }
    }

    Ok(())
}

pub fn list_assets_by_tag(storage: &Arc<dyn StorageEngine>, tag: &str) -> Result<()> {
    let records = storage.read_all_records()?;
    let active_tags = get_active_tags(&records.asset_tags);

    let assets: Vec<_> = active_tags
        .iter()
        .filter(|(_, tags)| tags.contains(tag))
        .map(|(asset, _)| asset.as_str())
        .collect();

    if assets.is_empty() {
        println!("No assets with tag: {}", tag);
    } else {
        println!("Assets tagged with '{}':", tag);
        for asset in &assets {
            println!("  • {}", asset);
        }
        println!("\nTotal: {} asset(s)", assets.len());
    }

    Ok(())
}

pub fn get_active_tags(all_tags: &[AssetTag]) -> BTreeMap<String, BTreeSet<String>> {
    let mut tags_map: BTreeMap<String, BTreeMap<String, Vec<&AssetTag>>> = BTreeMap::new();

    for tag in all_tags {
        tags_map
            .entry(tag.asset_name.clone())
            .or_default()
            .entry(tag.tag.clone())
            .or_default()
            .push(tag);
    }

    let mut active: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for (asset, tag_groups) in tags_map {
        for (tag_name, tag_records) in tag_groups {
            // Sort by timestamp to get latest state
            let mut sorted = tag_records.clone();
            sorted.sort_by_key(|t| t.tagged_at);
            
            // Check if last record is removal
            if let Some(last) = sorted.last() {
                if last.removed_at.is_none() {
                    // Active tag
                    active
                        .entry(asset.clone())
                        .or_default()
                        .insert(tag_name);
                }
            }
        }
    }

    active
}

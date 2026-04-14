use std::process;
use std::sync::Arc;

use crate::app::tags::*;
use crate::cli::TagCommand;
use crate::exit_codes::*;
use crate::storage::StorageEngine;

pub(crate) fn cmd_tag(tag_cmd: TagCommand, storage: &Arc<dyn StorageEngine>) {
    match tag_cmd {
        TagCommand::Add { asset_name, tag } => match add_tag(storage, &asset_name, &tag) {
            Ok(()) => process::exit(EXIT_SUCCESS),
            Err(e) => {
                eprintln!("Error adding tag: {}", e);
                process::exit(EXIT_GENERAL_ERROR);
            }
        },
        TagCommand::AddBulk { asset_names, tag } => {
            let mut success_count = 0;
            let mut error_count = 0;

            for asset_name in &asset_names {
                match add_tag(storage, asset_name, &tag) {
                    Ok(()) => {
                        println!("✓ Tagged '{}' with '{}'", asset_name, tag);
                        success_count += 1;
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to tag '{}': {}", asset_name, e);
                        error_count += 1;
                    }
                }
            }

            println!("\nBulk tagging complete:");
            println!("  Success: {}", success_count);
            if error_count > 0 {
                println!("  Failed:  {}", error_count);
            }

            if error_count > 0 {
                process::exit(EXIT_GENERAL_ERROR);
            } else {
                process::exit(EXIT_SUCCESS);
            }
        }
        TagCommand::Remove { asset_name, tag } => match remove_tag(storage, &asset_name, &tag) {
            Ok(()) => process::exit(EXIT_SUCCESS),
            Err(e) => {
                eprintln!("Error removing tag: {}", e);
                process::exit(EXIT_GENERAL_ERROR);
            }
        },
        TagCommand::List { asset_name } => match list_tags(storage, asset_name.as_deref()) {
            Ok(()) => process::exit(EXIT_SUCCESS),
            Err(e) => {
                eprintln!("Error listing tags: {}", e);
                process::exit(EXIT_GENERAL_ERROR);
            }
        },
        TagCommand::ListTags => match list_all_tag_names(storage) {
            Ok(()) => process::exit(EXIT_SUCCESS),
            Err(e) => {
                eprintln!("Error listing tags: {}", e);
                process::exit(EXIT_GENERAL_ERROR);
            }
        },
        TagCommand::ListByTag { tag } => match list_assets_by_tag(storage, &tag) {
            Ok(()) => process::exit(EXIT_SUCCESS),
            Err(e) => {
                eprintln!("Error listing assets: {}", e);
                process::exit(EXIT_GENERAL_ERROR);
            }
        },
    }
}

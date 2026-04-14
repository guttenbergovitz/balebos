use std::process;
use std::sync::Arc;

use crate::exit_codes::*;
use crate::storage::StorageEngine;

pub fn cmd_ui(storage: Arc<dyn StorageEngine>) {
    if let Err(e) = crate::tui::run(storage) {
        eprintln!("TUI error: {}", e);
        process::exit(EXIT_GENERAL_ERROR);
    }
    process::exit(EXIT_SUCCESS);
}

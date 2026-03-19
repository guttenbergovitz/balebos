use std::env;

pub enum ListOptions {
    All,
    Unique,
    None,
}

// Handle the 'list' command from CLI arguments
// Check if the command is 'list' and return options
pub fn handle_list_command() -> ListOptions {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        return ListOptions::None;
    }

    // Check if the first argument is "list"
    if args[1] == "list" {
        // Check for a --unique flag
        if args.len() >= 3 && args[2] == "--unique" {
            return ListOptions::Unique;
        }
        return ListOptions::All;
    }

    ListOptions::None
}

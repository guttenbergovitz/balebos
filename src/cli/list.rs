use std::env;

// Handle the 'list' command from CLI arguments
// Check if the command is 'list' and return a boolean accordingly
pub fn handle_list_command() -> bool {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        // Not enough arguments, but we provide a helpful message
        eprintln!("Usage: balebos list");
        return false;
    }

    // Check if the first argument is "list"
    args[1] == "list"
}

use std::env;

// Handle the 'stats' command from CLI arguments
pub fn handle_stats_command() -> bool {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        return false;
    }

    args[1] == "stats"
}

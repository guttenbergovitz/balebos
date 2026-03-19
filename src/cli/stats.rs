use std::env;

pub enum StatsOptions {
    Basic,
    ByDomain,
    None,
}

// Handle the 'stats' command from CLI arguments
pub fn handle_stats_command() -> StatsOptions {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        return StatsOptions::None;
    }

    if args[1] == "stats" {
        if args.len() >= 3 && args[2] == "--by-domain" {
            return StatsOptions::ByDomain;
        }
        return StatsOptions::Basic;
    }

    StatsOptions::None
}

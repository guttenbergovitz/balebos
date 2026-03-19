use std::env;

pub fn handle_scan_command() -> Option<String> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        return None;
    }

    if args[1] != "scan" {
        return None;
    }

    if args.len() < 3 {
        // Missing domain argument
        return None;
    }

    Some(args[2].clone())
}

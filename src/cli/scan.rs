use std::env;

pub fn handle_scan_command() -> Option<String> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: balebos scan <domain>");
        return None;
    }

    if args[1] != "scan" {
        return None;
    }

    Some(args[2].clone())
}

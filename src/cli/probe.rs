use std::env;

pub fn handle_probe_command() -> Option<(String, String)> {
    let args: Vec<String> = env::args().collect();

    // Need at least: balebos probe [subdomains|http] example.com
    if args.len() < 4 {
        return None;
    }

    if args[1] != "probe" {
        return None;
    }

    let probe_type = &args[2];
    let target = &args[3];

    if probe_type == "subdomains" || probe_type == "http" {
        Some((probe_type.clone(), target.clone()))
    } else {
        None
    }
}

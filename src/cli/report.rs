use std::env;

pub fn handle_report_command() -> bool {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        return false;
    }

    args[1] == "report"
}

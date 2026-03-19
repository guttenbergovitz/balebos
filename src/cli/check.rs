use std::env;

pub fn handle_check_command() -> bool {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        return false;
    }

    args[1] == "check"
}

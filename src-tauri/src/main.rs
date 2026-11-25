// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Card-idling helper mode: `--idle <appid> <steam_api64.dll>` (see idle_child.rs).
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--idle") {
        let code = free_stuff_hub_lib::idle_child_main(
            args.get(2).map(String::as_str).unwrap_or(""),
            args.get(3).map(String::as_str).unwrap_or(""),
        );
        std::process::exit(code);
    }
    free_stuff_hub_lib::run()
}

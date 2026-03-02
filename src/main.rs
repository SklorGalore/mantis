fn main() {
    if let Err(e) = mantis::tui::run_tui() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

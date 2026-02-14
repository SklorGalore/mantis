use log::{debug, info};
use std::fs;

fn main() {
    let log_file = fs::File::create("log.log").expect("Could not create log file");
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();

    info!("Beginning run...");

    let file_path = "cases/Hawaii40_20231026.RAW";
    let case_file = fs::read_to_string(file_path).expect("Could not read case file");
    debug!("Case file contents:\n {case_file}");
}

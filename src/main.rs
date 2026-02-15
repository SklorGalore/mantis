use log::info;
use std::fs;
use std::io::{self, BufRead};

fn main() {
    // establish log file as log.log
    let log_file = fs::File::create("log.log").expect("Could not create log file");

    // establish logger, default level debug, push output to log.log, initialize
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();

    info!("Beginning run...");

    // path to case
    let path = "cases/Hawaii40_20231026.RAW";
    read_case(path);
}

fn read_case(path: &str) {
    // store case as a string
    let file = fs::File::open(path).expect("Could not read file path.");

    // Consumes the iterator, returns an (Optional) String
    for line in io::BufReader::new(file).lines().map_while(Result::ok) {
        ()
    }
}

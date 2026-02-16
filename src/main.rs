use log::info;
use std::env;
use std::fs;

use mantis;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: mantis <path-to-raw-file>");
        std::process::exit(1);
    }
    let path = &args[1];

    // establish log file as log.log
    let log_file = fs::File::create("log.log").expect("Could not create log file");

    // establish logger, default level debug, push output to log.log, initialize
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();

    info!("Beginning run...");

    let network = mantis::load_flow::read_case_v33(path);

    info!(
        "Loaded case '{}': {} buses, {} loads, {} generators, {} branches",
        network.case_name,
        network.buses.len(),
        network.loads.len(),
        network.generators.len(),
        network.branches.len(),
    );

    println!("{}", network);
}

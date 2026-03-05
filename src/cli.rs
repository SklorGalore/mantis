use crate::case::Network;
use crate::parse::read_case_v33;
use std::io::{self, Write};

/// Runs the interactive command-line interface
pub fn run_cli() {
    let mut net: Option<Network> = None;

    println!("mantis - power systems analysis");
    println!("Type 'help' for available commands.\n");

    'cli: loop {
        print!("mantis> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() || input.is_empty() {
            break 'cli;
        }

        let parts: Vec<&str> = input.trim().split_whitespace().collect();
        if parts.is_empty() {
            continue 'cli;
        }

        // pick chosen input string
        match parts[0] {
            "open" => {
                if parts.len() < 2 {
                    // List available .raw files
                    println!("Available cases in cases/:");
                    match std::fs::read_dir("cases") {
                        Ok(entries) => {
                            for entry in entries.flatten() {
                                let name = entry.file_name();
                                let name = name.to_string_lossy();
                                if name.to_lowercase().ends_with(".raw") {
                                    println!("  {}", name);
                                }
                            }
                        }
                        Err(e) => println!("Could not read cases/: {}", e),
                    }
                    println!("Usage: open <filename>");
                    continue 'cli;
                }
                let filename = parts[1];
                let path = format!("cases/{}", filename);
                let n = read_case_v33(&path);
                println!(
                    "Loaded: {} ({} buses, {} branches, {} generators, {} loads)",
                    n.case_name,
                    n.buses.len(),
                    n.branches.len(),
                    n.generators.len(),
                    n.loads.len()
                );
                net = Some(n);
            }

            "solve" => {
                let Some(ref mut n) = net else {
                    println!("No case loaded. Use 'open <filename>' first.");
                    continue 'cli;
                };
                if n.dc_approximation() {
                    println!("DC load flow solved successfully.");
                } else {
                    println!("DC load flow failed.");
                }
            }

            "buses" => {
                let Some(ref n) = net else {
                    println!("No case loaded.");
                    continue 'cli;
                };
                println!(
                    "{:>5}  {:<14}  {:>4}  {:>8}  {:>9}  {:>10}  {:>10}",
                    "ID", "Name", "Type", "Vnom(kV)", "V(pu)", "Angle(deg)", "Pmis(MW)"
                );
                println!("{}", "-".repeat(75));
                for bus in &n.buses {
                    let (p_mis, _q_mis) = n.bus_mismatch(bus.bus_id);
                    println!(
                        "{:>5}  {:<14}  {:>4}  {:>8.2}  {:>9.6}  {:>10.4}  {:>10.3}",
                        bus.bus_id,
                        bus.bus_name,
                        bus.bus_type,
                        bus.nom_voltage,
                        bus.voltage,
                        bus.angle,
                        p_mis
                    );
                }
            }

            "branches" => {
                let Some(ref n) = net else {
                    println!("No case loaded.");
                    continue 'cli;
                };
                println!(
                    "{:>4}  {:>5}  {:>5}  {:>4}  {:>10}  {:>10}  {:>10}  {:>6}",
                    "ID", "From", "To", "Type", "R(pu)", "X(pu)", "Flow(MW)", "Status"
                );
                println!("{}", "-".repeat(70));
                for br in &n.branches {
                    println!(
                        "{:>4}  {:>5}  {:>5}  {:>4}  {:>10.6}  {:>10.6}  {:>10.2}  {:>6}",
                        br.id,
                        br.from_bus,
                        br.to_bus,
                        br.branch_type,
                        br.resistance,
                        br.reactance,
                        br.flow,
                        if br.branch_status { "ON" } else { "OFF" }
                    );
                }
            }

            "generators" => {
                let Some(ref n) = net else {
                    println!("No case loaded.");
                    continue 'cli;
                };
                println!(
                    "{:>4}  {:>5}  {:<16}  {:>10}  {:>10}  {:>6}",
                    "ID", "Bus", "Name", "P(MW)", "Q(MVAR)", "Status"
                );
                println!("{}", "-".repeat(60));
                for g in &n.generators {
                    println!(
                        "{:>4}  {:>5}  {:<16}  {:>10.3}  {:>10.3}  {:>6}",
                        g.gen_id,
                        g.gen_bus_id,
                        g.gen_name,
                        g.p_gen,
                        g.q_gen,
                        if g.gen_status { "ON" } else { "OFF" }
                    );
                }
            }

            "import" => {
                if parts.len() < 2 {
                    println!("Usage: import <file.json|file.bin>");
                    continue 'cli;
                }
                let path = parts[1];
                let result: Result<Network, String> = if path.ends_with(".bin") {
                    std::fs::read(path)
                        .map_err(|e| format!("Error reading file: {}", e))
                        .and_then(|bytes| {
                            bincode::deserialize(&bytes)
                                .map_err(|e| format!("Error parsing bincode: {}", e))
                        })
                } else {
                    std::fs::read_to_string(path)
                        .map_err(|e| format!("Error reading file: {}", e))
                        .and_then(|contents| {
                            serde_json::from_str(&contents)
                                .map_err(|e| format!("Error parsing JSON: {}", e))
                        })
                };
                match result {
                    Ok(mut n) => {
                        n.rebuild_bus_map();
                        println!(
                            "Imported: {} ({} buses, {} branches, {} generators, {} loads)",
                            n.case_name,
                            n.buses.len(),
                            n.branches.len(),
                            n.generators.len(),
                            n.loads.len()
                        );
                        net = Some(n);
                    }
                    Err(e) => println!("{}", e),
                }
            }

            "export" => {
                let Some(ref n) = net else {
                    println!("No case loaded.");
                    continue 'cli;
                };
                if parts.len() < 2 {
                    println!("Usage: export <file.json|file.bin>");
                    continue 'cli;
                }
                let path = parts[1];
                let result: Result<(), String> = if path.ends_with(".bin") {
                    bincode::serialize(n)
                        .map_err(|e| format!("Error serializing: {}", e))
                        .and_then(|bytes| {
                            std::fs::write(path, &bytes)
                                .map_err(|e| format!("Error writing file: {}", e))
                        })
                } else {
                    serde_json::to_string_pretty(n)
                        .map_err(|e| format!("Error serializing: {}", e))
                        .and_then(|json| {
                            std::fs::write(path, &json)
                                .map_err(|e| format!("Error writing file: {}", e))
                        })
                };
                match result {
                    Ok(_) => println!("Exported to {}", path),
                    Err(e) => println!("{}", e),
                }
            }

            "loads" => {
                let Some(ref n) = net else {
                    println!("No case loaded.");
                    continue 'cli;
                };
                println!(
                    "{:>4}  {:>5}  {:<20}  {:>10}  {:>10}",
                    "ID", "Bus", "Name", "P(MW)", "Q(MVAR)"
                );
                println!("{}", "-".repeat(55));
                for l in &n.loads {
                    println!(
                        "{:>4}  {:>5}  {:<20}  {:>10.3}  {:>10.3}",
                        l.load_id, l.bus_id, l.load_name, l.real_load, l.imag_load
                    );
                }
            }

            "help" => {
                println!("Commands:");
                println!("  open <file>   Load a RAW case from cases/ directory");
                println!("  open          List available case files");
                println!("  solve         Run DC load flow");
                println!("  buses         Print bus table");
                println!("  branches      Print branch table");
                println!("  generators    Print generator table");
                println!("  loads         Print load table");
                println!("  import <file> Load network from file (.json or .bin)");
                println!("  export <file> Export network to file (.json or .bin)");
                println!("  help          Show this help");
                println!("  quit / exit   Exit");
            }

            "quit" | "exit" => {
                println!("Goodbye.");
                break 'cli;
            }

            other => println!("Unknown command: '{}'. Type 'help' for commands.", other),
        }
    }
}

use std::fs;
use std::io::{self, BufRead};

use log::info;

use crate::case::*;

/// strip slashes or quotes from fields
fn strip_extras(s: &str) -> String {
    s.trim().trim_matches('\'').trim().to_string()
}

/// Parses a PSS/E RAW file into a Network.
pub fn read_case_v33(path: &str) -> Network {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to open {}: {}", path, e);
            std::process::exit(1);
        }
    };
    let lines: Vec<String> = io::BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .collect();

    // Parse header line (line 1): IC, SBASE, REV, XFRRAT, NXFRAT, BASFRQ / comment
    let header = &lines[0];
    let header_data = header.split('/').next().unwrap_or(header);
    let header_fields: Vec<&str> = header_data.split(',').collect();

    let s_base: f32 = header_fields[1].trim().parse().unwrap_or(100.0);
    let frequency: f32 = header_fields[5].trim().parse().unwrap_or(60.0);

    // Use the comment portion as the case name
    let case_name = header
        .split('/')
        .nth(1)
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    let mut network = Network::new(case_name, s_base, frequency);

    // Track which section we're in
    #[derive(PartialEq)]
    enum Section {
        Bus,
        Load,
        FixedShunt,
        Generator,
        Branch,
        Transformer,
        Done,
    }

    let mut section = Section::Bus;

    // skip 3 header lines
    let mut line_number = 3;

    // Branch, generator, and load ids should start at 1.
    let mut branch_index: usize = 0;
    let mut gen_index: usize = 0;
    let mut load_index: usize = 0;

    'lineloop: while line_number < lines.len() {
        let line = &lines[line_number];
        let trimmed = line.trim();

        // Check for end of file
        if trimmed.starts_with("Q") {
            break 'lineloop;
        }

        // Proceed to the next section (delimiter lines are "0 / ..." or "0 /...")
        if trimmed == "0" || trimmed.starts_with("0 /") || trimmed.starts_with("0 /") {
            section = match section {
                Section::Bus => Section::Load,
                Section::Load => Section::FixedShunt,
                Section::FixedShunt => Section::Generator,
                Section::Generator => Section::Branch,
                Section::Branch => Section::Transformer,
                Section::Transformer => Section::Done,
                Section::Done => Section::Done,
            };

            // No data to parse in section headers; move on.
            line_number += 1;
            continue;
        }

        if section == Section::Done {
            // Skip remaining sections (area, zone, owner, etc.)
            line_number += 1;
            continue;
        }

        if trimmed.is_empty() {
            line_number += 1;
            continue;
        }

        match section {
            Section::Bus => {
                // I, 'NAME', BASKV, IDE, AREA, ZONE, OWNER, VM, VA, NVHI, NVLO, EVHI, EVLO
                let fields: Vec<&str> = trimmed.split(',').collect();
                if fields.len() >= 13 {
                    let bus_id: usize = fields[0].trim().parse().unwrap_or(0);
                    let bus_name = strip_extras(fields[1]);
                    let nom_voltage: f32 = fields[2].trim().parse().unwrap_or(0.0);
                    let ide: u8 = fields[3].trim().parse().unwrap_or(1);
                    let voltage: f32 = fields[7].trim().parse().unwrap_or(1.0);
                    let angle: f32 = fields[8].trim().parse().unwrap_or(0.0);
                    let v_max_operating: f32 = fields[9].trim().parse().unwrap_or(1.1);
                    let v_min_operating: f32 = fields[10].trim().parse().unwrap_or(0.9);
                    let v_max_contingency: f32 = fields[11].trim().parse().unwrap_or(1.1);
                    let v_min_contingency: f32 = fields[12].trim().parse().unwrap_or(0.9);

                    let bus_type = match ide {
                        3 => BusType::Slack,
                        2 => BusType::PV,
                        1 => BusType::PQ,
                        4 => BusType::OOS,
                        _ => {
                            eprintln!("Unknown bus type code: {}", ide);
                            std::process::exit(1);
                        }
                    };

                    network.buses.push(Bus {
                        bus_id,
                        bus_name,
                        bus_type,
                        nom_voltage,
                        // bus is in service if the code is not 4
                        bus_status: ide != 4,
                        voltage,
                        angle,
                        real_shunt: 0.0,
                        imag_shunt: 0.0,
                        v_min_operating,
                        v_min_contingency,
                        v_max_operating,
                        v_max_contingency,
                    });
                }
            }

            Section::Load => {
                // I, 'ID', STATUS, AREA, ZONE, PL, QL, IP, IQ, YP, YQ, OWNER, SCALE, INTRPT
                let fields: Vec<&str> = trimmed.split(',').collect();
                if fields.len() >= 7 {
                    let bus_id: usize = fields[0].trim().parse().unwrap_or(0);
                    let name = strip_extras(fields[1]);
                    let status: u8 = fields[2].trim().parse().unwrap_or(1);
                    let pl: f32 = fields[5].trim().parse().unwrap_or(0.0);
                    let ql: f32 = fields[6].trim().parse().unwrap_or(0.0);

                    if status == 1 {
                        network.loads.push(Load {
                            load_id: load_index,
                            load_name: format!("Bus{}-{}", bus_id, name),
                            real_load: pl,
                            imag_load: ql,
                        });
                        load_index += 1;
                    }
                }
            }

            Section::FixedShunt => {
                // Skip fixed shunt data for now
            }

            Section::Generator => {
                // I, 'ID', PG, QG, QT, QB, VS, IREG, MBASE, ...
                let fields: Vec<&str> = trimmed.split(',').collect();
                if fields.len() >= 17 {
                    let bus_id: usize = fields[0].trim().parse().unwrap_or(0);
                    let name = strip_extras(fields[1]);
                    let pg: f32 = fields[2].trim().parse().unwrap_or(0.0);
                    let qg: f32 = fields[3].trim().parse().unwrap_or(0.0);
                    let qt: f32 = fields[4].trim().parse().unwrap_or(0.0);
                    let qb: f32 = fields[5].trim().parse().unwrap_or(0.0);
                    let vs: f32 = fields[6].trim().parse().unwrap_or(1.0);
                    let status: u8 = fields[14].trim().parse().unwrap_or(1);
                    let pt: f32 = fields[16].trim().parse().unwrap_or(0.0);
                    let pb: f32 = fields[17].trim().parse().unwrap_or(0.0);

                    network.generators.push(Generator {
                        gen_id: gen_index,
                        gen_bus_id: bus_id,
                        gen_name: format!("Bus{}-{}", bus_id, name),
                        gen_status: status == 1,
                        p_gen: pg,
                        q_gen: qg,
                        v_setpoint: vs,
                        p_min: pb,
                        p_max: pt,
                        q_min: qb,
                        q_max: qt,
                    });
                    gen_index += 1;
                }
            }

            Section::Branch => {
                // I, J, 'CKT', R, X, B, RATEA, RATEB, RATEC, GI, BI, GJ, BJ, ST, ...
                let fields: Vec<&str> = trimmed.split(',').collect();
                if fields.len() >= 14 {
                    let from_bus: usize = fields[0].trim().parse().unwrap_or(0);
                    let to_bus: usize = fields[1].trim().parse().unwrap_or(0);
                    // let name = strip_extras(fields[2]);
                    let r: f32 = fields[2].trim().parse().unwrap_or(0.0);
                    let x: f32 = fields[3].trim().parse().unwrap_or(0.0);
                    let b: f32 = fields[4].trim().parse().unwrap_or(0.0);
                    let rate_a: f32 = fields[5].trim().parse().unwrap_or(0.0);
                    let rate_b: f32 = fields[6].trim().parse().unwrap_or(0.0);
                    let gi: f32 = fields[9].trim().parse().unwrap_or(0.0);
                    let bi: f32 = fields[10].trim().parse().unwrap_or(0.0);
                    let gj: f32 = fields[11].trim().parse().unwrap_or(0.0);
                    let bj: f32 = fields[12].trim().parse().unwrap_or(0.0);
                    let status: u8 = fields[13].trim().parse().unwrap_or(1);

                    network.branches.push(Branch {
                        branch_type: BranchType::Line,
                        id: branch_index,
                        from_bus,
                        to_bus,
                        branch_name: String::from("            "),
                        branch_status: status == 1,
                        resistance: r,
                        reactance: x,
                        from_shunt_conductance: gi,
                        from_shunt_susceptance: bi + b / 2.0,
                        to_shunt_conductance: gj,
                        to_shunt_susceptance: bj + b / 2.0,
                        tap_ratio: 1.0,
                        phase_shift: 0.0,
                        operating_limit: rate_a,
                        contingency_limit: rate_b,
                    });
                    branch_index += 1;
                }
            }

            Section::Transformer => {
                // Two-winding transformer: 4 lines per record
                // Line 1: I, J, K, 'CKT', CW, CZ, CM, MAG1, MAG2, NMETR, 'NAME', STAT, ...
                // Line 2: R1-2, X1-2, SBASE1-2
                // Line 3: WINDV1, NOMV1, ANG1, RATA1, RATB1, RATC1, ...
                // Line 4: WINDV2, NOMV2
                let fields: Vec<&str> = trimmed.split(',').collect();
                if fields.len() < 5 {
                    line_number += 1;
                    continue;
                }

                let from_bus: usize = fields[0].trim().parse().unwrap_or(0);
                let to_bus: usize = fields[1].trim().parse().unwrap_or(0);
                let k: i32 = fields[2].trim().parse().unwrap_or(0);
                // let name = strip_extras(fields[3]);
                let status: u8 = fields[11].trim().parse().unwrap_or(1);

                let is_three_winding = k != 0;

                // Line 2: impedance data
                line_number += 1;
                if line_number >= lines.len() {
                    break;
                }
                let imp_line = lines[line_number].trim();
                let imp_fields: Vec<&str> = imp_line.split(',').collect();
                let r: f32 = imp_fields
                    .first()
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(0.0);
                let x: f32 = imp_fields
                    .get(1)
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(0.0);

                // Line 3: winding 1 data
                line_number += 1;
                if line_number >= lines.len() {
                    break;
                }
                let w1_line = lines[line_number].trim();
                let w1_fields: Vec<&str> = w1_line.split(',').collect();
                let tap_ratio: f32 = w1_fields
                    .first()
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(1.0);
                let angle: f32 = w1_fields
                    .get(2)
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(0.0);
                let rate_a: f32 = w1_fields
                    .get(3)
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(0.0);
                let rate_b: f32 = w1_fields
                    .get(4)
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(0.0);

                // Line 4: winding 2 data
                line_number += 1;
                if line_number >= lines.len() {
                    break;
                }

                // For three-winding transformers, there's a 5th line (winding 3)
                if is_three_winding {
                    line_number += 1;
                    if line_number >= lines.len() {
                        break;
                    }
                }

                network.branches.push(Branch {
                    branch_type: BranchType::TwoWinding,
                    id: branch_index,
                    from_bus,
                    to_bus,
                    branch_name: String::from("            "),
                    branch_status: status == 1,
                    resistance: r,
                    reactance: x,
                    from_shunt_conductance: 0.0,
                    from_shunt_susceptance: 0.0,
                    to_shunt_conductance: 0.0,
                    to_shunt_susceptance: 0.0,
                    tap_ratio,
                    phase_shift: angle,
                    operating_limit: rate_a,
                    contingency_limit: rate_b,
                });
                branch_index += 1;
            }

            Section::Done => {}
        }

        line_number += 1;
    }

    // Build bus_map: bus_id -> matrix index (excluding slack)
    let mut matrix_idx: usize = 0;
    for bus in &network.buses {
        if bus.bus_type != BusType::Slack {
            network.bus_map.insert(bus.bus_id, matrix_idx);
            matrix_idx += 1;
        }
    }

    info!(
        "Parsed {} buses, {} loads, {} generators, {} branches",
        network.buses.len(),
        network.loads.len(),
        network.generators.len(),
        network.branches.len(),
    );

    network
}

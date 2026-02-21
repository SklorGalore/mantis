use crate::case::*;

/// Serializes a Network into PSS/E v33 RAW format.
pub fn network_to_raw(net: &Network) -> String {
    let mut out = String::new();

    // Header line
    out.push_str(&format!(
        " 0, {:.1}, 33, 1.0, 1.0, {:.1} / {}\n",
        net.s_base, net.frequency, net.case_name
    ));
    // Two comment lines
    out.push_str("@  Exported by Mantis\n");
    out.push_str("@\n");

    // === Bus section ===
    for bus in &net.buses {
        let ide = match bus.bus_type {
            BusType::Slack => 3u8,
            BusType::PV => 2,
            BusType::PQ => 1,
            BusType::OOS => 4,
        };
        out.push_str(&format!(
            " {}, '{}', {:.1}, {}, 1, 1, 1, {:.6}, {:.6}, {:.4}, {:.4}, {:.4}, {:.4}\n",
            bus.bus_id,
            bus.bus_name,
            bus.nom_voltage,
            ide,
            bus.voltage,
            bus.angle,
            bus.v_max_operating,
            bus.v_min_operating,
            bus.v_max_contingency,
            bus.v_min_contingency,
        ));
    }
    out.push_str("0 / END OF BUS DATA\n");

    // === Load section ===
    for load in &net.loads {
        out.push_str(&format!(
            " {}, '1 ', 1, 1, 1, {:.3}, {:.3}, 0.000, 0.000, 0.000, 0.000, 1, 1, 0\n",
            load.bus_id, load.real_load, load.imag_load,
        ));
    }
    out.push_str("0 / END OF LOAD DATA\n");

    // === Fixed shunt section (empty) ===
    out.push_str("0 / END OF FIXED SHUNT DATA\n");

    // === Generator section ===
    for generator in &net.generators {
        let stat: u8 = if generator.gen_status { 1 } else { 0 };
        out.push_str(&format!(
            " {}, '1 ', {:.3}, {:.3}, {:.3}, {:.3}, {:.5}, 0, 100.0, 0.0, 1.0, 0.0, 1.0, 1.0, {}, 100.0, {:.3}, {:.3}\n",
            generator.gen_bus_id,
            generator.p_gen,
            generator.q_gen,
            generator.q_max,
            generator.q_min,
            generator.v_setpoint,
            stat,
            generator.p_max,
            generator.p_min,
        ));
    }
    out.push_str("0 / END OF GENERATOR DATA\n");

    // === Branch section (Lines only) ===
    for branch in &net.branches {
        if branch.branch_type != BranchType::Line {
            continue;
        }
        let stat: u8 = if branch.branch_status { 1 } else { 0 };
        // Reconstruct B from shunt susceptances (average of from+to)
        let b = branch.from_shunt_susceptance + branch.to_shunt_susceptance;
        out.push_str(&format!(
            " {}, {}, '1 ', {:.6}, {:.6}, {:.6}, {:.1}, {:.1}, 0.0, {:.6}, {:.6}, {:.6}, {:.6}, {}\n",
            branch.from_bus,
            branch.to_bus,
            branch.resistance,
            branch.reactance,
            b,
            branch.operating_limit,
            branch.contingency_limit,
            branch.from_shunt_conductance,
            branch.from_shunt_susceptance,
            branch.to_shunt_conductance,
            branch.to_shunt_susceptance,
            stat,
        ));
    }
    out.push_str("0 / END OF BRANCH DATA\n");

    // === Transformer section (TwoWinding branches) ===
    for branch in &net.branches {
        if branch.branch_type != BranchType::TwoWinding {
            continue;
        }
        let stat: u8 = if branch.branch_status { 1 } else { 0 };
        // Line 1
        out.push_str(&format!(
            " {}, {}, 0, '1 ', 1, 1, 1, 0.0, 0.0, 2, '            ', {}\n",
            branch.from_bus, branch.to_bus, stat,
        ));
        // Line 2: R, X, SBASE1-2
        out.push_str(&format!(
            " {:.6}, {:.6}, 100.0\n",
            branch.resistance, branch.reactance,
        ));
        // Line 3: WINDV1, NOMV1, ANG1, RATA1, RATB1, RATC1, ...
        out.push_str(&format!(
            " {:.4}, 0.0, {:.4}, {:.1}, {:.1}, 0.0, 0, 0, 1.1, 0.9, 1.1, 0.9, 33, 0, 0.0, 0.0\n",
            branch.tap_ratio, branch.phase_shift, branch.operating_limit, branch.contingency_limit,
        ));
        // Line 4: WINDV2, NOMV2
        out.push_str(" 1.0, 0.0\n");
    }
    out.push_str("0 / END OF TRANSFORMER DATA\n");

    out.push_str("Q\n");
    out
}

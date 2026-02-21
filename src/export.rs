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

    // === Fixed shunt section ===
    for shunt in &net.fixed_shunts {
        let stat: u8 = if shunt.status { 1 } else { 0 };
        out.push_str(&format!(
            " {}, '{}', {}, {:.3}, {:.3}\n",
            shunt.bus_id, shunt.shunt_id, stat, shunt.gl, shunt.bl,
        ));
    }
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

    // === Area section ===
    for area in &net.areas {
        out.push_str(&format!(
            " {}, {}, {:.2}, {:.2}, '{}'\n",
            area.area_id, area.slack_bus, area.p_desired, area.p_tolerance, area.area_name,
        ));
    }
    out.push_str("0 / END OF AREA DATA\n");

    // === Two-terminal DC, VSC DC, impedance correction, multi-terminal DC,
    //     multi-section line, inter-area transfer, owner, FACTS sections (empty) ===
    out.push_str("0 / END OF TWO-TERMINAL DC DATA\n");
    out.push_str("0 / END OF VSC DC LINE DATA\n");
    out.push_str("0 / END OF IMPEDANCE CORRECTION DATA\n");
    out.push_str("0 / END OF MULTI-TERMINAL DC DATA\n");
    out.push_str("0 / END OF MULTI-SECTION LINE DATA\n");

    // === Zone section ===
    for zone in &net.zones {
        out.push_str(&format!(" {}, '{}'\n", zone.zone_id, zone.zone_name));
    }
    out.push_str("0 / END OF ZONE DATA\n");

    out.push_str("0 / END OF INTER-AREA TRANSFER DATA\n");
    out.push_str("0 / END OF OWNER DATA\n");
    out.push_str("0 / END OF FACTS DEVICE DATA\n");

    // === Switched shunt section ===
    for shunt in &net.switched_shunts {
        let stat: u8 = if shunt.status { 1 } else { 0 };
        let mut line = format!(
            " {}, {}, 0, {}, {:.4}, {:.4}, {}, 100.0, '            ', {:.4}",
            shunt.bus_id, shunt.modsw, stat, shunt.v_hi, shunt.v_lo, shunt.remote_bus, shunt.b_init,
        );
        for (n, b) in &shunt.steps {
            line.push_str(&format!(", {}, {:.4}", n, b));
        }
        line.push('\n');
        out.push_str(&line);
    }
    out.push_str("0 / END OF SWITCHED SHUNT DATA\n");

    out.push_str("Q\n");
    out
}

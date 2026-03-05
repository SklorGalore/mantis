use crate::case::*;
use rsparse::data::Trpl;

impl Network {
    /// Runs DC load flow and writes bus angles and branch flows directly into the network.
    /// Returns true on success, false on failure.
    pub fn dc_approximation(&mut self) -> bool {
        self.rebuild_bus_map();
        let n = self.bus_map.len();

        if n == 0 {
            return false;
        }

        // initialize the admittance matrix
        let mut b_prime = Trpl::<f64>::new();
        b_prime.m = n;
        b_prime.n = n;

        // Collect OUT bus IDs for quick lookup
        let out_buses: std::collections::HashSet<usize> = self.buses.iter()
            .filter(|b| b.bus_type == BusType::OUT)
            .map(|b| b.bus_id)
            .collect();

        for branch in &self.branches {
            if !branch.branch_status || branch.reactance == 0.0 {
                continue;
            }

            // Skip branches connected to out-of-service buses
            if out_buses.contains(&branch.from_bus) || out_buses.contains(&branch.to_bus) {
                continue;
            }

            let bij = 1.0 / branch.reactance as f64;

            let from = self.bus_map.get(&branch.from_bus);
            let to = self.bus_map.get(&branch.to_bus);

            // B'_ii += 1/X, B'_jj += 1/X, B'_ij -= 1/X, B'_ji -= 1/X
            if let (Some(&i), Some(&j)) = (from, to) {
                b_prime.append(i, i, bij);
                b_prime.append(j, j, bij);
                b_prime.append(i, j, -bij);
                b_prime.append(j, i, -bij);
            } else if let Some(&i) = from {
                b_prime.append(i, i, bij);
            } else if let Some(&j) = to {
                b_prime.append(j, j, bij);
            }
        }
        b_prime.sum_dupl();

        // Build P injection vector (in per unit)
        let mut p = vec![0.0f64; n];

        for generator in &self.generators {
            if generator.gen_status {
                if let Some(&idx) = self.bus_map.get(&generator.gen_bus_id) {
                    p[idx] += generator.p_gen as f64 / self.s_base as f64;
                }
            }
        }

        for load in &self.loads {
            if let Some(&idx) = self.bus_map.get(&load.bus_id) {
                p[idx] -= load.real_load as f64 / self.s_base as f64;
            }
        }

        // Solve B' * theta = P
        let csc = b_prime.to_sprs();
        if rsparse::lusol(&csc, &mut p, 0, 1e-6).is_err() {
            return false;
        }
        // p now contains theta (radians) for each non-slack bus

        // Write bus angles (degrees) directly into bus structs
        for bus in &mut self.buses {
            if bus.bus_type == BusType::OUT {
                bus.voltage = 0.0;
                bus.angle = 0.0;
            } else if bus.bus_type == BusType::Slack {
                bus.angle = 0.0;
            } else if let Some(&idx) = self.bus_map.get(&bus.bus_id) {
                bus.angle = p[idx].to_degrees() as f32;
            }
        }

        // Compute and write branch flows (MW) directly into branch structs
        for branch in &mut self.branches {
            if !branch.branch_status || branch.reactance == 0.0
                || out_buses.contains(&branch.from_bus) || out_buses.contains(&branch.to_bus)
            {
                branch.flow = 0.0;
                continue;
            }

            let theta_i = self
                .bus_map
                .get(&branch.from_bus)
                .map(|&idx| p[idx])
                .unwrap_or(0.0);

            let theta_j = self
                .bus_map
                .get(&branch.to_bus)
                .map(|&idx| p[idx])
                .unwrap_or(0.0);

            branch.flow = ((theta_i - theta_j) / branch.reactance as f64 * self.s_base as f64) as f32;
        }

        // Back-calculate slack bus generator output from branch flows
        // Slack P_gen = P_load_at_slack + sum(flows leaving slack)
        let slack_ids: Vec<usize> = self.buses.iter()
            .filter(|b| b.bus_type == BusType::Slack)
            .map(|b| b.bus_id)
            .collect();

        for &slack_id in &slack_ids {
            let p_load: f32 = self.loads.iter()
                .filter(|l| l.bus_id == slack_id)
                .map(|l| l.real_load)
                .sum();

            let p_flow_out: f32 = self.branches.iter()
                .filter(|br| br.branch_status)
                .map(|br| {
                    if br.from_bus == slack_id { br.flow }
                    else if br.to_bus == slack_id { -br.flow }
                    else { 0.0 }
                })
                .sum();

            // Total required generation at this bus
            let p_required = p_load + p_flow_out;

            // Sum of other (non-first) generators on this bus
            let slack_gens: Vec<usize> = self.generators.iter()
                .enumerate()
                .filter(|(_, g)| g.gen_bus_id == slack_id && g.gen_status)
                .map(|(i, _)| i)
                .collect();

            if let Some(&first) = slack_gens.first() {
                let other_gen: f32 = slack_gens.iter()
                    .skip(1)
                    .map(|&i| self.generators[i].p_gen)
                    .sum();
                self.generators[first].p_gen = p_required - other_gen;
            }
        }

        true
    }
}

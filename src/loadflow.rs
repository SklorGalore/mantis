use crate::case::*;
use log::debug;
use rsparse::data::Trpl;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DcSolution {
    pub bus_angles: HashMap<usize, f64>, // bus_id -> angle (degrees)
    pub branch_flows: HashMap<usize, f64>, // branch.id -> MW flow
}

impl Network {
    pub fn dc_approximation(&self) -> Option<DcSolution> {
        let n = self.bus_map.len(); // number of non-slack buses
        debug!("Found {:>6} non-slack buses", n);

        if n == 0 {
            return None;
        }

        // initialize the admittance matrix
        let mut b_prime = Trpl::<f64>::new();
        b_prime.m = n;
        b_prime.n = n;

        for branch in &self.branches {
            if !branch.branch_status || branch.reactance == 0.0 {
                continue;
            }

            // mutual admittance Y_ij = -sum(admittance between bus i and j)
            let b = -1.0 / branch.reactance as f64;
            debug!(
                "b matrix entry for branch from {} to {} is {}",
                branch.from_bus, branch.to_bus, b
            );

            // admittance matrix indices for from and to bus
            let from = self.bus_map.get(&branch.from_bus);
            let to = self.bus_map.get(&branch.to_bus);

            if let (Some(&i), Some(&j)) = (from, to) {
                debug!("i: {:>6}, j: {:>6}", i, j);
                // both non-slack
                b_prime.append(i, i, b);
                b_prime.append(j, j, b);
                b_prime.append(i, j, -b);
                b_prime.append(j, i, -b);
            } else if let Some(&i) = from {
                // to is slack — diagonal only
                b_prime.append(i, i, b);
            } else if let Some(&j) = to {
                // from is slack — diagonal only
                b_prime.append(j, j, b);
            }
        }
        b_prime.sum_dupl();
        debug!("B'=\n{:?}", b_prime.to_sprs().to_dense());

        // Build P injection vector (in per unit)
        let mut p = vec![0.0f64; n];

        // Add generator injections
        for generator in &self.generators {
            if generator.gen_status {
                if let Some(&idx) = self.bus_map.get(&generator.gen_bus_id) {
                    p[idx] += generator.p_gen as f64 / self.s_base as f64;
                }
            }
        }

        // Subtract load injections
        for load in &self.loads {
            if let Some(&idx) = self.bus_map.get(&load.bus_id) {
                p[idx] -= load.real_load as f64 / self.s_base as f64;
            }
        }

        // Solve B' * theta = P (theta in radians, stored back in p)
        let csc = b_prime.to_sprs();
        if let Err(e) = rsparse::lusol(&csc, &mut p, 0, 1e-6) {
            debug!("DC solve failed: {:?}", e);
            return None;
        }
        // p now contains theta (radians) for each non-slack bus

        // Build bus_angles map (degrees)
        let mut bus_angles = HashMap::new();

        // Slack buses are at 0.0 degrees
        for bus in &self.buses {
            if bus.bus_type == BusType::Slack {
                bus_angles.insert(bus.bus_id, 0.0);
            }
        }

        // Non-slack buses: look up index in bus_map
        for bus in &self.buses {
            if let Some(&idx) = self.bus_map.get(&bus.bus_id) {
                bus_angles.insert(bus.bus_id, p[idx].to_degrees());
            }
        }

        // Compute branch flows (MW)
        let mut branch_flows = HashMap::new();
        for branch in &self.branches {
            if !branch.branch_status || branch.reactance == 0.0 {
                continue;
            }

            let theta_i = self
                .bus_map
                .get(&branch.from_bus)
                .map(|&idx| p[idx])
                .unwrap_or(0.0); // slack bus = 0 radians

            let theta_j = self
                .bus_map
                .get(&branch.to_bus)
                .map(|&idx| p[idx])
                .unwrap_or(0.0); // slack bus = 0 radians

            let flow = (theta_i - theta_j) / branch.reactance as f64 * self.s_base as f64;
            branch_flows.insert(branch.id, flow);
        }

        Some(DcSolution {
            bus_angles,
            branch_flows,
        })
    }
}

use crate::case::*;
use log::debug;
use nalgebra::DMatrix;

impl Network {
    pub fn fast_decoupled(&self) {
        let n = self.bus_map.len(); // number of non-slack buses
        debug!("Found {:>6} non-slack buses", n);

        // initialize the admittance matrix
        let mut b_prime = DMatrix::<f32>::zeros(n, n);

        for branch in &self.branches {
            if !branch.branch_status {
                continue;
            }

            // mutual admittance Y_ij = -sum(admittance between bus i and j)
            let b = -1.0 / branch.reactance;
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
                b_prime[(i, i)] += b;
                b_prime[(j, j)] += b;
                b_prime[(i, j)] -= b;
                b_prime[(j, i)] -= b;
            } else if let Some(&i) = from {
                // to is slack — diagonal only
                b_prime[(i, i)] += b;
            } else if let Some(&j) = to {
                // from is slack — diagonal only
                b_prime[(j, j)] += b;
            }
        }
        debug!("B'=\n{b_prime}");
    }
}

use crate::case::*;
use log::debug;
use nalgebra::{DMatrix, DVector};
use num_complex::Complex;
use rsparse::data::Trpl;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DcSolution {
    pub bus_angles: HashMap<usize, f64>, // bus_id -> angle (degrees)
    pub branch_flows: HashMap<usize, f64>, // branch.id -> MW flow
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcSolution {
    pub bus_voltages: HashMap<usize, f64>, // bus_id -> voltage magnitude (pu)
    pub bus_angles: HashMap<usize, f64>,   // bus_id -> voltage angle (degrees)
    pub log: Vec<String>,
}

impl Network {
    /** Solves a given network using the DC load flow approximation.
     * The DC approximation neglects line resistances and bus voltages/angles.
     * This method is fast, but loses accuracy for heavily loaded transmission
     * systems and is not appropriate for distribution systems or networks with
     * a low X/R ratio.
     */
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
    pub fn decoupled(&self) -> Option<AcSolution> {
        Some(AcSolution {
            bus_voltages: HashMap::new(),
            bus_angles: HashMap::new(),
            log: Vec::new(),
        })
    }

    /// Solves the network using the Newton-Raphson power flow method.
    pub fn newton_raphson_solution(&self) -> AcSolution {
        let mut log_messages: Vec<String> = Vec::new();
        log_messages.push(format!("Starting Newton-Raphson power flow solution."));

        // Constants
        let tolerance = 1e-6;
        let max_iterations = 100;

        // Number of buses
        let num_buses = self.buses.len();
        if num_buses == 0 {
            log_messages.push(format!("No buses found in the network."));
            return AcSolution { bus_voltages: HashMap::new(), bus_angles: HashMap::new(), log: log_messages };
        }

        // Identify bus types and count PV/PQ buses
        let mut pv_buses = Vec::new();
        let mut pq_buses = Vec::new();
        let mut slack_bus_id = None;

        for bus in &self.buses {
            match bus.bus_type {
                BusType::Slack => {
                    if slack_bus_id.is_some() {
                        log_messages.push(format!(
                            "Multiple slack buses found, which is not allowed for Newton-Raphson."
                        ));
                        return AcSolution { bus_voltages: HashMap::new(), bus_angles: HashMap::new(), log: log_messages };
                    }
                    slack_bus_id = Some(bus.bus_id);
                }
                BusType::PV => pv_buses.push(bus.bus_id),
                BusType::PQ => pq_buses.push(bus.bus_id),
                _ => {} // Ignore OOS buses
            }
        }

        let slack_bus_id = match slack_bus_id {
            Some(id) => id,
            None => {
                log_messages.push(format!("No slack bus found, which is required for Newton-Raphson."));
                return AcSolution { bus_voltages: HashMap::new(), bus_angles: HashMap::new(), log: log_messages };
            }
        };

        log_messages.push(format!("Slack bus ID: {}", slack_bus_id));
        log_messages.push(format!("PV buses: {:?}", pv_buses));
        log_messages.push(format!("PQ buses: {:?}", pq_buses));

        let _num_pv_pq = pv_buses.len() + pq_buses.len();
        let _num_pq = pq_buses.len();

        let mut bus_idx_map: HashMap<usize, usize> = HashMap::new();
        for (idx, bus) in self.buses.iter().enumerate() {
            bus_idx_map.insert(bus.bus_id, idx);
        }

        // Calculate scheduled P and Q for each bus (P_gen - P_load, Q_gen - Q_load) in pu
        let mut p_scheduled = HashMap::new();
        let mut q_scheduled = HashMap::new();

        for bus in &self.buses {
            p_scheduled.insert(bus.bus_id, 0.0);
            q_scheduled.insert(bus.bus_id, 0.0);
        }

        for generator_data in &self.generators {
            if generator_data.gen_status {
                *p_scheduled.get_mut(&generator_data.gen_bus_id).unwrap() +=
                    generator_data.p_gen as f64 / self.s_base as f64;
                *q_scheduled.get_mut(&generator_data.gen_bus_id).unwrap() +=
                    generator_data.q_gen as f64 / self.s_base as f64;
            }
        }

        for load in &self.loads {
            *p_scheduled.get_mut(&load.bus_id).unwrap() -=
                load.real_load as f64 / self.s_base as f64;
            *q_scheduled.get_mut(&load.bus_id).unwrap() -=
                load.imag_load as f64 / self.s_base as f64;
        }

        // Initialize voltage magnitudes and angles
        let mut v = HashMap::new(); // Voltage magnitudes in pu
        let mut delta = HashMap::new(); // Voltage angles in radians

        for bus in &self.buses {
            // Slack bus: V = 1.0 pu, angle = 0.0 radians
            if bus.bus_id == slack_bus_id {
                v.insert(bus.bus_id, 1.0);
                delta.insert(bus.bus_id, 0.0);
            } else {
                v.insert(bus.bus_id, bus.voltage as f64);
                delta.insert(bus.bus_id, bus.angle as f64 * std::f64::consts::PI / 180.0);
            }
        }

        // Build Y-bus matrix
        let y_bus = self.build_ybus_matrix(&bus_idx_map);
        log_messages.push(format!("Y-bus matrix built."));

        // Iteration loop
        for iter in 0..max_iterations {
            log_messages.push(format!("Newton-Raphson iteration: {}", iter + 1));

            let mut p_calc = vec![0.0; num_buses];
            let mut q_calc = vec![0.0; num_buses];

            for bus_idx_i in 0..num_buses {
                let bus_id_i = self.buses[bus_idx_i].bus_id;
                let v_i = v[&bus_id_i];
                let delta_i = delta[&bus_id_i];

                let mut sum_p = 0.0;
                let mut sum_q = 0.0;

                for bus_idx_k in 0..num_buses {
                    let bus_id_k = self.buses[bus_idx_k].bus_id;
                    let v_k = v[&bus_id_k];
                    let delta_k = delta[&bus_id_k];

                    let y_ik = y_bus[(bus_idx_i, bus_idx_k)];
                    let g_ik = y_ik.re;
                    let b_ik = y_ik.im;

                    let angle_diff = delta_i - delta_k;

                    sum_p += v_k * (g_ik * angle_diff.cos() + b_ik * angle_diff.sin());
                    sum_q += v_k * (g_ik * angle_diff.sin() - b_ik * angle_diff.cos());
                }
                p_calc[bus_idx_i] = v_i * sum_p;
                q_calc[bus_idx_i] = v_i * sum_q;
            }

            // Calculate mismatches
            let mut delta_p_q = Vec::new(); // Mismatch vector

            // Delta P for all non-slack buses
            for bus in &self.buses {
                if bus.bus_id == slack_bus_id {
                    continue;
                }
                let bus_idx = bus_idx_map[&bus.bus_id];
                let mismatch = p_scheduled[&bus.bus_id] - p_calc[bus_idx];
                delta_p_q.push(mismatch);
            }

            // Delta Q for PQ buses only
            for bus in &self.buses {
                if bus.bus_type == BusType::PQ {
                    let bus_idx = bus_idx_map[&bus.bus_id];
                    let mismatch = q_scheduled[&bus.bus_id] - q_calc[bus_idx];
                    delta_p_q.push(mismatch);
                }
            }

            // Create index mappings for Jacobian
            let mut non_slack_bus_to_idx = HashMap::new();
            let mut pq_bus_to_idx = HashMap::new();
            let mut current_non_slack_idx = 0;
            let mut current_pq_idx = 0;

            for bus in &self.buses {
                if bus.bus_id == slack_bus_id {
                    continue;
                }
                non_slack_bus_to_idx.insert(bus.bus_id, current_non_slack_idx);
                current_non_slack_idx += 1;
                if bus.bus_type == BusType::PQ {
                    pq_bus_to_idx.insert(bus.bus_id, current_pq_idx);
                    current_pq_idx += 1;
                }
            }

            let num_non_slack = current_non_slack_idx;
            let num_pq = current_pq_idx;

            // Initialize Jacobian sub-matrices
            let mut j11 = DMatrix::zeros(num_non_slack, num_non_slack);
            let mut j12 = DMatrix::zeros(num_non_slack, num_pq);
            let mut j21 = DMatrix::zeros(num_pq, num_non_slack);
            let mut j22 = DMatrix::zeros(num_pq, num_pq);

            // Populate Jacobian sub-matrices
            for bus_i_data in &self.buses {
                if bus_i_data.bus_id == slack_bus_id {
                    continue;
                }
                let i = *non_slack_bus_to_idx.get(&bus_i_data.bus_id).unwrap();
                let bus_i_id = bus_i_data.bus_id;
                let v_i = v[&bus_i_id];
                let delta_i = delta[&bus_i_id];
                let p_i_calc = p_calc[bus_idx_map[&bus_i_id]];
                let q_i_calc = q_calc[bus_idx_map[&bus_i_id]];

                for bus_k_data in &self.buses {
                    if bus_k_data.bus_id == slack_bus_id {
                        continue;
                    }
                    let k = *non_slack_bus_to_idx.get(&bus_k_data.bus_id).unwrap();
                    let bus_k_id = bus_k_data.bus_id;
                    let v_k = v[&bus_k_id];
                    let delta_k = delta[&bus_k_id];

                    let y_ik = y_bus[(bus_idx_map[&bus_i_id], bus_idx_map[&bus_k_id])];
                    let g_ik = y_ik.re;
                    let b_ik = y_ik.im;

                    let angle_diff = delta_i - delta_k;

                    // dP/ddelta (J11)
                    if i == k {
                        // Diagonal element
                        j11[(i, k)] = -q_i_calc - v_i * v_k * b_ik;
                    } else {
                        // Off-diagonal element
                        j11[(i, k)] = -v_i * v_k * (g_ik * angle_diff.sin() - b_ik * angle_diff.cos());
                    }

                    // dP/dV (J12)
                    if bus_k_data.bus_type == BusType::PQ {
                        let k_pq = *pq_bus_to_idx.get(&bus_k_data.bus_id).unwrap();
                        if i == k {
                            // Diagonal element
                            j12[(i, k_pq)] = p_i_calc / v_i + v_i * g_ik;
                        } else {
                            // Off-diagonal element
                            j12[(i, k_pq)] = v_i * (g_ik * angle_diff.cos() + b_ik * angle_diff.sin());
                        }
                    }

                    // dQ/ddelta (J21)
                    if bus_i_data.bus_type == BusType::PQ {
                        let i_pq = *pq_bus_to_idx.get(&bus_i_data.bus_id).unwrap();
                        if i == k {
                            // Diagonal element
                            j21[(i_pq, k)] = p_i_calc - v_i * v_k * g_ik;
                        } else {
                            // Off-diagonal element
                            j21[(i_pq, k)] = v_i * v_k * (g_ik * angle_diff.cos() + b_ik * angle_diff.sin());
                        }
                    }

                    // dQ/dV (J22)
                    if bus_i_data.bus_type == BusType::PQ && bus_k_data.bus_type == BusType::PQ {
                        let i_pq = *pq_bus_to_idx.get(&bus_i_data.bus_id).unwrap();
                        let k_pq = *pq_bus_to_idx.get(&bus_k_data.bus_id).unwrap();
                        if i == k {
                            // Diagonal element
                            j22[(i_pq, k_pq)] = q_i_calc / v_i - v_i * b_ik;
                        } else {
                            // Off-diagonal element
                            j22[(i_pq, k_pq)] = v_i * (g_ik * angle_diff.sin() - b_ik * angle_diff.cos());
                        }
                    }
                }
            }
            let jacobian =
                DMatrix::from_fn(num_non_slack + num_pq, num_non_slack + num_pq, |r, c| {
                    if r < num_non_slack && c < num_non_slack {
                        j11[(r, c)]
                    } else if r < num_non_slack && c >= num_non_slack {
                        j12[(r, c - num_non_slack)]
                    } else if r >= num_non_slack && c < num_non_slack {
                        j21[(r - num_non_slack, c)]
                    } else {
                        j22[(r - num_non_slack, c - num_non_slack)]
                    }
                });
            log_messages.push(format!("Jacobian matrix formed for iteration {}.", iter + 1));

            // Solve for corrections
            let delta_p_q_vector = DVector::from_vec(delta_p_q);

            let delta_x = match jacobian.try_inverse() {
                Some(j_inv) => j_inv * delta_p_q_vector,
                None => {
                    log_messages.push(format!("Jacobian is singular, cannot solve for corrections."));
                    return AcSolution { bus_voltages: HashMap::new(), bus_angles: HashMap::new(), log: log_messages };
                }
            };

            log_messages.push(format!("Corrections calculated for iteration {}.", iter + 1));

            // Update voltage magnitudes and angles
            let mut max_mismatch: f64 = 0.0;
            let mut current_delta_x_idx = 0;

            for bus_data in &self.buses {
                if bus_data.bus_id == slack_bus_id {
                    continue;
                }

                let bus_id = bus_data.bus_id;

                // Update angles (for all non-slack buses)
                let delta_delta = delta_x[current_delta_x_idx];
                *delta.get_mut(&bus_id).unwrap() += delta_delta;
                max_mismatch = max_mismatch.max(delta_delta.abs());
                current_delta_x_idx += 1; // Move to the next correction

                // Update voltage magnitudes (for PQ buses only)
                if bus_data.bus_type == BusType::PQ {
                    let delta_v_div_v = delta_x[current_delta_x_idx];
                    *v.get_mut(&bus_id).unwrap() *= 1.0 + delta_v_div_v;
                    max_mismatch = max_mismatch.max(delta_v_div_v.abs());
                    current_delta_x_idx += 1; // Move to the next correction
                }
            }

            log_messages.push(format!("Max mismatch for iteration {}: {:.6}", iter + 1, max_mismatch));

            // Check for convergence
            if max_mismatch < tolerance {
                log_messages.push(format!("Newton-Raphson converged in {} iterations.", iter + 1));
                // Prepare and return AcSolution
                let mut bus_voltages = HashMap::new();
                let mut bus_angles = HashMap::new();

                for bus in &self.buses {
                    let bus_id = bus.bus_id;
                    bus_voltages.insert(bus_id, v[&bus_id]);
                    bus_angles.insert(bus_id, delta[&bus_id].to_degrees());
                }

                return AcSolution {
                    bus_voltages,
                    bus_angles,
                    log: log_messages,
                };
            }
        }

        log_messages.push(format!(
            "Newton-Raphson failed to converge after {} iterations.",
            max_iterations
        ));
        AcSolution { bus_voltages: HashMap::new(), bus_angles: HashMap::new(), log: log_messages }
    }

    fn build_ybus_matrix(&self, bus_idx_map: &HashMap<usize, usize>) -> DMatrix<Complex<f64>> {
        let num_buses = self.buses.len();

        let mut ybus_g_trpl = Trpl::<f64>::new();
        let mut ybus_b_trpl = Trpl::<f64>::new();
        ybus_g_trpl.m = num_buses;
        ybus_g_trpl.n = num_buses;
        ybus_b_trpl.m = num_buses;
        ybus_b_trpl.n = num_buses;

        for branch in &self.branches {
            if !branch.branch_status {
                continue;
            }

            let from_idx = *bus_idx_map.get(&branch.from_bus).unwrap();
            let to_idx = *bus_idx_map.get(&branch.to_bus).unwrap();

            // Series admittance calculation
            let r = branch.resistance as f64;
            let x = branch.reactance as f64;
            let z = Complex::new(r, x);
            let y_series = if z.norm_sqr() == 0.0 {
                Complex::new(0.0, 0.0)
            } else {
                1.0 / z
            };

            // Half-line charging susceptance (if applicable, from Branch are half-line charging)
            let b_c_from = branch.from_shunt_susceptance as f64;
            let b_c_to = branch.to_shunt_susceptance as f64;

            let y_shunt_from = Complex::new(0.0, b_c_from);
            let y_shunt_to = Complex::new(0.0, b_c_to);

            // Add to Y-bus (real and imaginary parts separately)
            // Diagonal elements
            ybus_g_trpl.append(from_idx, from_idx, (y_series + y_shunt_from).re);
            ybus_b_trpl.append(from_idx, from_idx, (y_series + y_shunt_from).im);
            ybus_g_trpl.append(to_idx, to_idx, (y_series + y_shunt_to).re);
            ybus_b_trpl.append(to_idx, to_idx, (y_series + y_shunt_to).im);
            // Off-diagonal elements
            ybus_g_trpl.append(from_idx, to_idx, -y_series.re);
            ybus_b_trpl.append(from_idx, to_idx, -y_series.im);
            ybus_g_trpl.append(to_idx, from_idx, -y_series.re);
            ybus_b_trpl.append(to_idx, from_idx, -y_series.im);
        }

        // Add fixed shunts (gl + jbl)
        for shunt in &self.fixed_shunts {
            if !shunt.status {
                continue;
            }
            let bus_idx = *bus_idx_map.get(&shunt.bus_id).unwrap();
            let g = shunt.gl as f64 / self.s_base as f64; // Convert from MW to pu
            let b = shunt.bl as f64 / self.s_base as f64; // Convert from MVAR to pu
            ybus_g_trpl.append(bus_idx, bus_idx, g);
            ybus_b_trpl.append(bus_idx, bus_idx, b);
        }

        // TODO: Add switched shunts.

        ybus_g_trpl.sum_dupl(); // Sum duplicate entries for real part
        ybus_b_trpl.sum_dupl(); // Sum duplicate entries for imaginary part

        let sparse_ybus_g = ybus_g_trpl.to_sprs();
        let sparse_ybus_b = ybus_b_trpl.to_sprs();

        let g_matrix_vec_vec = sparse_ybus_g.to_dense();
        let b_matrix_vec_vec = sparse_ybus_b.to_dense();

        // Convert Vec<Vec<f64>> to DMatrix<f64>
        let g_matrix = DMatrix::from_fn(num_buses, num_buses, |r, c| g_matrix_vec_vec[r][c]);
        let b_matrix = DMatrix::from_fn(num_buses, num_buses, |r, c| b_matrix_vec_vec[r][c]);

        // Combine into a DMatrix of Complex numbers
        DMatrix::from_fn(num_buses, num_buses, |r, c| {
            Complex::new(g_matrix[(r, c)], b_matrix[(r, c)])
        })
    }
}
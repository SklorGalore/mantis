use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

fn is_zero(v: &f32) -> bool { *v == 0.0 }
fn is_true(v: &bool) -> bool { *v }
fn is_empty_str(v: &String) -> bool { v.is_empty() }
fn default_true() -> bool { true }
fn default_one() -> f32 { 1.0 }
fn default_v_min_op() -> f32 { 0.9 }
fn default_v_min_ct() -> f32 { 0.95 }
fn default_v_max_op() -> f32 { 1.05 }
fn default_v_max_ct() -> f32 { 1.1 }

/// Bus type enum.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BusType {
    Slack, // slack, swing, Vd, reference bus
    PQ,    // load bus
    PV,    // generator bus
    OUT,   // out of service
}

/// Display the bus type as a string.
impl fmt::Display for BusType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BusType::Slack => write!(f, "REF"),
            BusType::PQ => write!(f, "P-Q"),
            BusType::PV => write!(f, "P-V"),
            BusType::OUT => write!(f, "OOS"),
        }
    }
}

/// Bus struct with the necessary fields for a power system bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bus {
    pub bus_id: usize,
    pub bus_name: String,
    pub bus_type: BusType,
    #[serde(skip_serializing_if = "is_zero")]
    pub nom_voltage: f32,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub bus_status: bool,

    #[serde(default = "default_one", skip_serializing_if = "is_zero")]
    pub voltage: f32,
    #[serde(skip_serializing_if = "is_zero")]
    pub angle: f32,

    #[serde(skip_serializing_if = "is_zero")]
    pub real_shunt: f32,
    #[serde(skip_serializing_if = "is_zero")]
    pub imag_shunt: f32,

    #[serde(default = "default_v_min_op", skip_serializing_if = "is_zero")]
    pub v_min_operating: f32,
    #[serde(default = "default_v_min_ct", skip_serializing_if = "is_zero")]
    pub v_min_contingency: f32,
    #[serde(default = "default_v_max_op", skip_serializing_if = "is_zero")]
    pub v_max_operating: f32,
    #[serde(default = "default_v_max_ct", skip_serializing_if = "is_zero")]
    pub v_max_contingency: f32,
}

/// Bus struct constructor.
impl Bus {
    pub fn new(bus_id: usize, bus_name: String, bus_type: BusType) -> Self {
        Self {
            bus_id,
            bus_name,
            bus_status: bus_type != BusType::OUT,
            bus_type,
            nom_voltage: 0.0,
            voltage: 1.0,
            angle: 0.0,
            real_shunt: 0.0,
            imag_shunt: 0.0,
            v_min_operating: 0.9,
            v_min_contingency: 0.95,
            v_max_operating: 1.05,
            v_max_contingency: 1.1,
        }
    }
}

/// Bus struct display implementation.
impl fmt::Display for Bus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Bus {:>3} {:<14} {:>5} {:>8.2} kV  |V|={:.6}  Angle={:>9.6}",
            self.bus_id, self.bus_name, self.bus_type, self.nom_voltage, self.voltage, self.angle
        )
    }
}

/// Load struct constructor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Load {
    pub load_id: usize,
    pub bus_id: usize,
    pub load_name: String,

    pub real_load: f32,
    pub imag_load: f32,
}

/// Load struct display implementation.
impl Load {
    pub fn new(
        load_id: usize,
        bus_id: usize,
        load_name: String,
        real_load: f32,
        imag_load: f32,
    ) -> Self {
        Self {
            load_id,
            bus_id,
            load_name,
            real_load,
            imag_load,
        }
    }
}

/// Load struct display implementation.
impl fmt::Display for Load {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Load {:>3} {:<20} P={:>9.3} MW  Q={:>9.3} MVAR",
            self.load_id, self.load_name, self.real_load, self.imag_load
        )
    }
}

/// BranchType enum display implementation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BranchType {
    Line,
    TwoWinding,
}

/// BranchType enum display implementation.
impl fmt::Display for BranchType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BranchType::Line => write!(f, "Line"),
            BranchType::TwoWinding => write!(f, "Xfmr"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub branch_type: BranchType,
    pub id: usize,
    pub from_bus: usize,
    pub to_bus: usize,
    #[serde(skip_serializing_if = "is_empty_str")]
    pub branch_name: String,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub branch_status: bool,

    #[serde(skip_serializing_if = "is_zero")]
    pub resistance: f32,
    pub reactance: f32,
    #[serde(skip_serializing_if = "is_zero")]
    pub from_shunt_conductance: f32,
    #[serde(skip_serializing_if = "is_zero")]
    pub from_shunt_susceptance: f32,
    #[serde(skip_serializing_if = "is_zero")]
    pub to_shunt_conductance: f32,
    #[serde(skip_serializing_if = "is_zero")]
    pub to_shunt_susceptance: f32,

    #[serde(default = "default_one", skip_serializing_if = "is_zero")]
    pub tap_ratio: f32,
    #[serde(skip_serializing_if = "is_zero")]
    pub phase_shift: f32,

    #[serde(skip_serializing_if = "is_zero")]
    pub operating_limit: f32,
    #[serde(skip_serializing_if = "is_zero")]
    pub contingency_limit: f32,

    #[serde(skip_serializing_if = "is_zero")]
    pub flow: f32,
}

impl Branch {
    pub fn new(
        id: usize,
        from_bus: usize,
        to_bus: usize,
        branch_type: BranchType,
        resistance: f32,
        reactance: f32,
    ) -> Self {
        Self {
            id,
            from_bus,
            to_bus,
            branch_type,
            branch_name: String::new(),
            branch_status: true,
            resistance,
            reactance,
            from_shunt_conductance: 0.0,
            from_shunt_susceptance: 0.0,
            to_shunt_conductance: 0.0,
            to_shunt_susceptance: 0.0,
            tap_ratio: 1.0,
            phase_shift: 0.0,
            operating_limit: 0.0,
            contingency_limit: 0.0,
            flow: 0.0,
        }
    }
}

impl fmt::Display for Branch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Type: {:<4} Id: {:>3} Name: {:<16} From->To: {:>3} -> {:<3}  R={:>10.6}  X={:>10.6}  Tap={:.4}  RateA={:>7.1} RateB={:>7.1}",
            self.branch_type,
            self.id,
            self.branch_name,
            self.from_bus,
            self.to_bus,
            self.resistance,
            self.reactance,
            self.tap_ratio,
            self.operating_limit,
            self.contingency_limit,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Generator {
    pub gen_id: usize,
    pub gen_bus_id: usize,
    pub gen_name: String,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub gen_status: bool,

    #[serde(skip_serializing_if = "is_zero")]
    pub p_gen: f32,
    #[serde(skip_serializing_if = "is_zero")]
    pub q_gen: f32,
    #[serde(default = "default_one", skip_serializing_if = "is_zero")]
    pub v_setpoint: f32,

    #[serde(skip_serializing_if = "is_zero")]
    pub p_min: f32,
    #[serde(skip_serializing_if = "is_zero")]
    pub p_max: f32,
    #[serde(skip_serializing_if = "is_zero")]
    pub q_min: f32,
    #[serde(skip_serializing_if = "is_zero")]
    pub q_max: f32,
}

impl Generator {
    pub fn new(gen_id: usize, gen_bus_id: usize, gen_name: String) -> Self {
        Self {
            gen_id,
            gen_bus_id,
            gen_name,
            gen_status: true,
            p_gen: 0.0,
            q_gen: 0.0,
            v_setpoint: 1.0,
            p_min: 0.0,
            p_max: 0.0,
            q_min: 0.0,
            q_max: 0.0,
        }
    }
}

impl fmt::Display for Generator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Gen {:>3} {:<16} Bus {:>3}  P={:>9.3} MW  Q={:>9.3} MVAR  Vset={:.5}",
            self.gen_id, self.gen_name, self.gen_bus_id, self.p_gen, self.q_gen, self.v_setpoint
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Network {
    pub case_name: String,
    pub s_base: f32,
    pub frequency: f32,

    pub buses: Vec<Bus>,
    pub branches: Vec<Branch>,
    pub loads: Vec<Load>,
    pub generators: Vec<Generator>,
    #[serde(skip)]
    pub bus_map: HashMap<usize, usize>, // bus_id -> matrix index (slack excluded)
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "Case: {}  Sbase: {} MVA  Frequency: {} Hz",
            self.case_name, self.s_base, self.frequency
        )?;
        writeln!(
            f,
            "{} buses, {} loads, {} generators, {} branches\n",
            self.buses.len(),
            self.loads.len(),
            self.generators.len(),
            self.branches.len()
        )?;

        writeln!(f, "=== Buses ===")?;
        for bus in &self.buses {
            writeln!(f, "  {}", bus)?;
        }

        writeln!(f, "\n=== Loads ===")?;
        for load in &self.loads {
            writeln!(f, "  {}", load)?;
        }

        writeln!(f, "\n=== Generators ===")?;
        for generator in &self.generators {
            writeln!(f, "  {}", generator)?;
        }

        writeln!(f, "\n=== Branches ===")?;
        for branch in &self.branches {
            writeln!(f, "  {}", branch)?;
        }

        Ok(())
    }
}

impl Network {
    // New case
    pub fn new(case_name: String, s_base: f32, frequency: f32) -> Self {
        Self {
            case_name,
            s_base,
            frequency,
            buses: Vec::new(),
            branches: Vec::new(),
            loads: Vec::new(),
            generators: Vec::new(),
            bus_map: HashMap::new(),
        }
    }

    /// Compute (P_mismatch, Q_mismatch) for a given bus.
    /// P_mis = P_gen - P_load - P_flow_out
    /// Q_mis = Q_gen - Q_load
    pub fn bus_mismatch(&self, bus_id: usize) -> (f32, f32) {
        let p_gen: f32 = self
            .generators
            .iter()
            .filter(|g| g.gen_bus_id == bus_id && g.gen_status)
            .map(|g| g.p_gen)
            .sum();
        let q_gen: f32 = self
            .generators
            .iter()
            .filter(|g| g.gen_bus_id == bus_id && g.gen_status)
            .map(|g| g.q_gen)
            .sum();
        let p_load: f32 = self
            .loads
            .iter()
            .filter(|l| l.bus_id == bus_id)
            .map(|l| l.real_load)
            .sum();
        let q_load: f32 = self
            .loads
            .iter()
            .filter(|l| l.bus_id == bus_id)
            .map(|l| l.imag_load)
            .sum();
        let p_flow_out: f32 = self
            .branches
            .iter()
            .filter(|br| br.branch_status)
            .map(|br| {
                if br.from_bus == bus_id {
                    br.flow
                } else if br.to_bus == bus_id {
                    -br.flow
                } else {
                    0.0
                }
            })
            .sum();
        (p_gen - p_load - p_flow_out, q_gen - q_load)
    }

    /// Rebuild bus_map from current buses list (must be called after any bus change)
    pub fn rebuild_bus_map(&mut self) {
        self.bus_map.clear();
        let mut matrix_idx: usize = 0;
        for bus in &self.buses {
            if bus.bus_type != BusType::Slack && bus.bus_type != BusType::OUT {
                self.bus_map.insert(bus.bus_id, matrix_idx);
                matrix_idx += 1;
            }
        }
    }
}

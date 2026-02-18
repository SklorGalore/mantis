use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BusType {
    Slack, // slack, swing, Vd, reference bus
    PQ,    // load bus
    PV,    // generator bus
    OOS,   // out of service
}

impl fmt::Display for BusType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BusType::Slack => write!(f, "REF"),
            BusType::PQ => write!(f, "P-Q"),
            BusType::PV => write!(f, "P-V"),
            BusType::OOS => write!(f, "OOS"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Bus {
    // Identifiers
    pub bus_id: usize,
    pub bus_name: String,
    pub bus_type: BusType,
    pub nom_voltage: f32,
    pub bus_status: bool,

    // Voltage
    pub voltage: f32,
    pub angle: f32,

    // Shunts
    pub real_shunt: f32,
    pub imag_shunt: f32,

    // Limits
    pub v_min_operating: f32,
    pub v_min_contingency: f32,
    pub v_max_operating: f32,
    pub v_max_contingency: f32,
}

impl fmt::Display for Bus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Bus {:>3} {:<14} {:>5} {:>8.2} kV  |V|={:.6}  Angle={:>9.6}",
            self.bus_id, self.bus_name, self.bus_type, self.nom_voltage, self.voltage, self.angle
        )
    }
}

pub struct Load {
    pub load_id: usize,
    pub load_name: String,

    pub real_load: f32,
    pub imag_load: f32,
}

impl fmt::Display for Load {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Load {:>3} {:<20} P={:>9.3} MW  Q={:>9.3} MVAR",
            self.load_id, self.load_name, self.real_load, self.imag_load
        )
    }
}

pub enum BranchType {
    Line,
    TwoWinding,
}

impl fmt::Display for BranchType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BranchType::Line => write!(f, "Line"),
            BranchType::TwoWinding => write!(f, "Xfmr"),
        }
    }
}

pub struct Branch {
    // Identifiers
    pub branch_type: BranchType,
    pub id: usize,
    pub from_bus: usize,
    pub to_bus: usize,
    pub branch_name: String,
    pub branch_status: bool,

    // Impedance data
    pub resistance: f32,
    pub reactance: f32,
    pub from_shunt_conductance: f32,
    pub from_shunt_susceptance: f32,
    pub to_shunt_conductance: f32,
    pub to_shunt_susceptance: f32,

    // Transformer data
    pub tap_ratio: f32,
    pub phase_shift: f32,

    // Limits
    pub operating_limit: f32,
    pub contingency_limit: f32,
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

#[derive(Debug, Clone)]
pub struct Generator {
    // Identifiers
    pub gen_id: usize,
    pub gen_bus_id: usize,
    pub gen_name: String,
    pub gen_status: bool,

    // Setpoints
    pub p_gen: f32,
    pub q_gen: f32,
    pub v_setpoint: f32,

    // Limits
    pub p_min: f32,
    pub p_max: f32,
    pub q_min: f32,
    pub q_max: f32,
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

pub struct Network {
    pub case_name: String,
    pub s_base: f32,
    pub frequency: f32,

    pub buses: Vec<Bus>,
    pub branches: Vec<Branch>,
    pub loads: Vec<Load>,
    pub generators: Vec<Generator>,
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
}

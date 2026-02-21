use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
impl Bus {
    pub fn new(bus_id: usize, bus_name: String, bus_type: BusType) -> Self {
        Self {
            bus_id,
            bus_name,
            bus_status: bus_type != BusType::OOS,
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

impl fmt::Display for Bus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Bus {:>3} {:<14} {:>5} {:>8.2} kV  |V|={:.6}  Angle={:>9.6}",
            self.bus_id, self.bus_name, self.bus_type, self.nom_voltage, self.voltage, self.angle
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Load {
    pub load_id: usize,
    pub bus_id: usize,
    pub load_name: String,

    pub real_load: f32,
    pub imag_load: f32,
}

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

impl fmt::Display for Load {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Load {:>3} {:<20} P={:>9.3} MW  Q={:>9.3} MVAR",
            self.load_id, self.load_name, self.real_load, self.imag_load
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Fixed shunt at a bus (PSS/E Fixed Shunt section).
/// GL and BL are in MW and MVAR respectively at 1.0 pu voltage on the system MVA base.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixedShunt {
    pub bus_id: usize,
    pub shunt_id: String,
    pub status: bool,
    pub gl: f32, // active component in MW at 1.0 pu voltage
    pub bl: f32, // reactive component in MVAR at 1.0 pu voltage (positive = capacitive)
}

impl fmt::Display for FixedShunt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "FixedShunt Bus {:>3} ID {:<4} GL={:>8.3} MW  BL={:>8.3} MVAR",
            self.bus_id, self.shunt_id, self.gl, self.bl
        )
    }
}

/// Switched shunt (capacitor/reactor bank) at a bus.
/// Steps holds (count, susceptance_per_step_pu) pairs for each bank block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchedShunt {
    pub bus_id: usize,
    pub modsw: u8,       // 0=fixed, 1=discrete, 2=continuous, …
    pub status: bool,
    pub v_hi: f32,       // VSWHI: upper voltage limit (pu)
    pub v_lo: f32,       // VSWLO: lower voltage limit (pu)
    pub remote_bus: usize, // SWREM: 0 = local control
    pub b_init: f32,     // BINIT: initial susceptance (pu on system base)
    pub steps: Vec<(i32, f32)>, // (Nx, Bx) step banks, up to 8 pairs
}

impl SwitchedShunt {
    /// Total minimum susceptance available (most capacitive bank fully in)
    pub fn b_max(&self) -> f32 {
        self.steps
            .iter()
            .map(|(n, b)| if *n > 0 { *n as f32 * b } else { 0.0 })
            .sum()
    }

    /// Total minimum susceptance available (most inductive bank fully in)
    pub fn b_min(&self) -> f32 {
        self.steps
            .iter()
            .map(|(n, b)| if *n < 0 { *n as f32 * b } else { 0.0 })
            .sum()
    }
}

impl fmt::Display for SwitchedShunt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SwitchedShunt Bus {:>3}  Binit={:>8.4} pu  Vlo={:.4}  Vhi={:.4}  Banks={}",
            self.bus_id,
            self.b_init,
            self.v_lo,
            self.v_hi,
            self.steps.len()
        )
    }
}

/// Power system area with interchange scheduling data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Area {
    pub area_id: usize,
    pub slack_bus: usize,  // ISW: area slack bus (0 = use system slack)
    pub p_desired: f32,    // PDES: desired net export in MW
    pub p_tolerance: f32,  // PTOL: interchange tolerance in MW
    pub area_name: String,
}

impl fmt::Display for Area {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Area {:>3} {:<20} Slack={:>4}  Pdes={:>8.1} MW  Ptol={:.1} MW",
            self.area_id, self.area_name, self.slack_bus, self.p_desired, self.p_tolerance
        )
    }
}

/// Electrical zone (grouping of buses for zonal analysis).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Zone {
    pub zone_id: usize,
    pub zone_name: String,
}

impl fmt::Display for Zone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Zone {:>3} {}", self.zone_id, self.zone_name)
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
    pub fixed_shunts: Vec<FixedShunt>,
    pub switched_shunts: Vec<SwitchedShunt>,
    pub areas: Vec<Area>,
    pub zones: Vec<Zone>,
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
            "{} buses, {} loads, {} generators, {} branches, {} fixed shunts, {} switched shunts, {} areas, {} zones\n",
            self.buses.len(),
            self.loads.len(),
            self.generators.len(),
            self.branches.len(),
            self.fixed_shunts.len(),
            self.switched_shunts.len(),
            self.areas.len(),
            self.zones.len(),
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

        if !self.fixed_shunts.is_empty() {
            writeln!(f, "\n=== Fixed Shunts ===")?;
            for shunt in &self.fixed_shunts {
                writeln!(f, "  {}", shunt)?;
            }
        }

        if !self.switched_shunts.is_empty() {
            writeln!(f, "\n=== Switched Shunts ===")?;
            for shunt in &self.switched_shunts {
                writeln!(f, "  {}", shunt)?;
            }
        }

        if !self.areas.is_empty() {
            writeln!(f, "\n=== Areas ===")?;
            for area in &self.areas {
                writeln!(f, "  {}", area)?;
            }
        }

        if !self.zones.is_empty() {
            writeln!(f, "\n=== Zones ===")?;
            for zone in &self.zones {
                writeln!(f, "  {}", zone)?;
            }
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
            fixed_shunts: Vec::new(),
            switched_shunts: Vec::new(),
            areas: Vec::new(),
            zones: Vec::new(),
            bus_map: HashMap::new(),
        }
    }

    /// Rebuild bus_map from current buses list (must be called after any bus change)
    pub fn rebuild_bus_map(&mut self) {
        self.bus_map.clear();
        let mut matrix_idx: usize = 0;
        for bus in &self.buses {
            if bus.bus_type != BusType::Slack {
                self.bus_map.insert(bus.bus_id, matrix_idx);
                matrix_idx += 1;
            }
        }
    }
}

mod load_flow {
    use core::f32;

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum BusType {
        Slack,
        PQ,
        PV,
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

    pub struct Load {
        pub load_id: usize,
        pub load_name: String,

        pub real_load: f32,
        pub imag_load: f32,
    }

    pub enum BranchType {
        Line,
        TwoWinding,
    }

    pub struct Branch {
        // Identifiers
        pub branch_code: BranchType,
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

    #[derive(Debug, Clone)]
    pub struct Generator {
        // Identifiers
        pub gen_id: usize,
        pub gen_bus_id: usize,
        pub gen_name: String,
        pub gen_status: bool,

        // Setpoints
        pub p_gen: f64,
        pub q_gen: f64,
        pub v_setpoint: f64,

        // Limits
        pub p_min: f64,
        pub p_max: f64,
        pub q_min: f64,
        pub q_max: f64,
    }

    pub struct Network {
        pub case_name: String,
        pub s_base: f32,
        pub frequency: f32,

        pub buses: Vec<Bus>,
        pub branches: Vec<Branch>,
        pub loads: Vec<Load>,
        pub generators: Vec<Generator>,
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
            }
        }
    }
}

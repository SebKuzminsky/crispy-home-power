/// State from the Varta battery pack.
#[derive(Debug, Clone)]
pub struct VartaState {
    pub soc: f32,
    pub charge_current_request: f32,
    pub charge_voltage_request: f32,
    pub voltage: f32,
    pub current: f32,
}

impl VartaState {
    pub fn eq_approx(&self, other: &Self) -> bool {
        (self.soc - other.soc).abs() < 0.01
            && (self.charge_current_request - other.charge_current_request).abs() < 0.01
            && (self.charge_voltage_request - other.charge_voltage_request).abs() < 0.01
            && (self.voltage - other.voltage).abs() < 0.01
    }
}

/// State from the PV system / Enphase Envoy.
#[derive(Debug, Clone)]
pub struct PvState {
    pub grid_export_watts: f32,
}

impl PvState {
    pub fn eq_approx(&self, other: &Self) -> bool {
        (self.grid_export_watts - other.grid_export_watts).abs() < 1.0
    }
}

/// Command to the Delta-Q charger.
#[derive(Debug, Clone)]
pub struct ChargerCommand {
    pub on: bool,
    pub voltage: f32,
    pub current: f32,
}

impl ChargerCommand {
    pub fn eq_approx(&self, other: &Self) -> bool {
        self.on == other.on
            && (self.voltage - other.voltage).abs() < 0.01
            && (self.current - other.current).abs() < 0.01
    }
}

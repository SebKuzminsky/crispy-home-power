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

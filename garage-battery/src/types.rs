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

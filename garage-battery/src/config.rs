use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub varta_can_interface: String,
    pub charger_can_interface: String,
    pub envoy_url: String,
    pub envoy_auth_token_path: String,
    pub pv_poll_interval_secs: f64,
    pub charger_command_interval_secs: f64,
    pub control: ControlConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ControlConfig {
    pub excess_pv_min_soc: f32,
    pub excess_pv_max_soc: f32,
    pub no_export_min_soc: f32,
    pub no_export_max_soc: f32,
    pub default_charge_current: f32,
    pub charger_max_dc_current: f32,
    pub export_margin_w: f32,
}

pub fn load(config_path: &str) -> Config {
    let content = std::fs::read_to_string(config_path)
        .unwrap_or_else(|e| panic!("cannot read config file '{}': {}", config_path, e));
    toml::from_str(&content)
        .unwrap_or_else(|e| panic!("cannot parse config file '{}': {}", config_path, e))
}

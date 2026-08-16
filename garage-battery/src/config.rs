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
pub struct ControlConfigSolar {
    pub charger_max_dc_current: f32,
    pub excess_pv_min_soc: f32,
    pub excess_pv_max_soc: f32,
    pub no_excess_min_soc: f32,
    pub no_excess_max_soc: f32,
    pub export_margin_w: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ControlConfigHold {
    pub charger_max_dc_current: f32,
    pub soc: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub enum ControlConfig {
    Solar(ControlConfigSolar),
    Hold(ControlConfigHold),
}

pub fn load(config_path: &str) -> Config {
    let content = std::fs::read_to_string(config_path)
        .unwrap_or_else(|e| panic!("cannot read config file '{}': {}", config_path, e));
    toml::from_str(&content)
        .unwrap_or_else(|e| panic!("cannot parse config file '{}': {}", config_path, e))
}

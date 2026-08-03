use enphase_local::Envoy;
use reqwest::Url;

use crate::types::PvState;

#[derive(Debug)]
struct WhReading {
    wh_lifetime: f32,
    timestamp: chrono::DateTime<chrono::Utc>,
}

pub async fn run(
    envoy_url: &str,
    auth_token: &str,
    poll_interval_secs: f64,
    state_tx: tokio::sync::watch::Sender<Option<PvState>>,
) {
    let url = Url::parse(envoy_url).expect("invalid envoy URL");

    let envoy = Envoy::new(url, auth_token);
    let interval = tokio::time::Duration::from_secs_f64(poll_interval_secs);

    let mut prev_wh_reading: Option<WhReading> = None;

    let mut last_state: Option<PvState> = None;

    loop {
        let wh_reading = match poll_envoy(&envoy).await {
            Ok(val) => val,
            Err(e) => {
                println!("pv_monitor: error polling envoy: {}", e);
                tokio::time::sleep(interval).await;
                continue;
            },
        };
        // println!("polled wh_reading={wh_reading:#?}");
        // println!("prev wh_reading={prev_wh_reading:#?}");

        let grid_export_watts = match prev_wh_reading {
            None => {
                prev_wh_reading = Some(wh_reading);
                continue;
            },
            Some(prev_wh_reading) => {
                //
                let time_delta_s =
                    (wh_reading.timestamp - prev_wh_reading.timestamp).num_seconds() as f32;
                let wh_imported = wh_reading.wh_lifetime - prev_wh_reading.wh_lifetime;
                let ws_imported = wh_imported * 60.0 * 60.0;
                -ws_imported / time_delta_s
            },
        };
        prev_wh_reading = Some(wh_reading);
        // println!("watts exported: {grid_export_watts:.1} W");

        let state = PvState { grid_export_watts };

        let is_new = match &last_state {
            Some(prev) => !state.eq_approx(prev),
            None => true,
        };

        if is_new {
            // println!("pv_monitor: export={:.0}W", state.grid_export_watts);
            last_state = Some(state.clone());
            let _ = state_tx.send(Some(state));
        }

        tokio::time::sleep(interval).await;
    }
}

async fn poll_envoy(envoy: &Envoy) -> Result<WhReading, Box<dyn std::error::Error + Send + Sync>> {
    let production = envoy.production().await?;

    let net_consumption_device = production.consumption.into_iter().find(|device| {
        device.type_ == enphase_local::production::DeviceType::Eim
            && device.measurement_type.unwrap()
                == enphase_local::production::MeasurementType::NetConsumption
    });

    if let Some(net_consumption_device) = net_consumption_device {
        // println!("net consumption device: {net_consumption_device:#?}");
        if let Some(net_consumption_details) = net_consumption_device.details {
            // println!("details: {net_consumption_details:#?}");
            return Ok(WhReading {
                wh_lifetime: net_consumption_details.wh_lifetime as f32,
                timestamp: net_consumption_device.reading_time,
            });
        }
    }

    Err(Box::new(std::io::Error::other(
        "failed to parse Enphase Envoy data",
    )))
}

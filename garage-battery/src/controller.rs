use crate::config::ControlConfig;
use crate::types::{ChargerCommand, PvState};
use crate::varta::VartaState;

pub async fn run(
    config: ControlConfig,
    mut varta_rx: tokio::sync::watch::Receiver<Option<VartaState>>,
    mut pv_rx: tokio::sync::watch::Receiver<Option<PvState>>,
    command_tx: tokio::sync::watch::Sender<Option<ChargerCommand>>,
) {
    let mut last_command: Option<ChargerCommand> = None;
    let mut command = ChargerCommand { on: false, voltage: 0.0, current: 0.0 };

    // This is how much the batteries are asking for. This gets clipped
    // by the available current before getting sent to the charger.
    let mut desired_charge_current: f32 = 0.0;

    // This is an upper bound on charge current imposed by the
    // availability of power. If we're on solar it's whatever surplus
    // we're generating. If we're on grid power it's infinity.
    let mut max_charge_current: f32 = 0.0;

    loop {
        tokio::select! {
            _ = varta_rx.changed() => {
                // println!("new varta state: {:#?}", varta_rx.borrow());
            }
            _ = pv_rx.changed() => {
                // println!("new pv state: {:#?}", pv_rx.borrow());
            }
        }

        let varta_state = varta_rx.borrow().clone();
        let pv_state = pv_rx.borrow().clone();

        let (varta_state, pv_state) = match (varta_state, pv_state) {
            (Some(v), Some(p)) => (v, p),
            _ => continue,
        };

        //
        // Update the charger command.
        //

        let exporting = pv_state.grid_export_watts > 0.0;

        println!("desired charge current: {desired_charge_current:.1} A");

        let (min_soc, max_soc, available_current) = if exporting {
            println!("exporting {} W", pv_state.grid_export_watts);
            (
                config.excess_pv_min_soc,
                config.excess_pv_max_soc,
                if varta_state.voltage > 0.0 {
                    (pv_state.grid_export_watts - config.export_margin_w) / varta_state.voltage
                } else {
                    config.default_charge_current
                },
            )
        } else {
            println!("not exporting enough ({} W)", pv_state.grid_export_watts);
            (
                config.no_export_min_soc,
                config.no_export_max_soc,
                f32::INFINITY,
            )
        };

        println!("available current: {:.1} A", available_current);

        if available_current > 0.0 {
            // There is more current available...
            if desired_charge_current > max_charge_current {
                // ... And we want more, so take a little more.
                max_charge_current += available_current * 0.1;
            }
        } else {
            // We're already drawing more than we should, reduce a bit.
            max_charge_current += available_current * 0.1;
        }
        max_charge_current = max_charge_current.clamp(0.0, config.charger_max_dc_current);
        println!("max charge current: {:.1} A", max_charge_current);

        if varta_state.soc >= max_soc {
            command.on = false;
            command.voltage = 0.0;
            command.current = 0.0;
            desired_charge_current = 0.0;
            max_charge_current = 0.0;
        } else if varta_state.soc <= min_soc {
            command.on = true;
        }

        command.voltage = varta_state.charge_voltage_request;

        if command.on {
            let current_error = varta_state.current - varta_state.charge_current_request;
            desired_charge_current -= current_error * 0.1;
            desired_charge_current =
                desired_charge_current.clamp(0.0, varta_state.charge_current_request);
            command.current = desired_charge_current.clamp(0.0, max_charge_current);
        } else {
            command.current = 0.0;
            desired_charge_current = 0.0;
        }

        // println!(
        //     "controller: command on={} voltage={:.3} current={:.3}",
        //     command.on, command.voltage, command.current
        // );

        let is_new = match &last_command {
            Some(prev) => !command.eq_approx(prev),
            None => true,
        };

        if is_new {
            last_command = Some(command.clone());
            let _ = command_tx.send(Some(command.clone()));
        }
    }
}

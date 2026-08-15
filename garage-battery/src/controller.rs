use crate::config::ControlConfig;
use crate::types::PvState;

pub async fn run(
    config: ControlConfig,
    mut varta_rx: tokio::sync::broadcast::Receiver<crate::varta::Message>,
    mut pv_rx: tokio::sync::watch::Receiver<Option<PvState>>,
    command_tx: tokio::sync::watch::Sender<Option<crate::charger::ChargerCommand>>,
) {
    let mut last_command: Option<crate::charger::ChargerCommand> = None;
    let mut command = crate::charger::ChargerCommand { on: false, voltage: 0.0, current: 0.0 };

    // This is how much the batteries are asking for. This gets clipped
    // by the available current before getting sent to the charger.
    let mut desired_charge_current: f32 = 0.0;

    // This is an upper bound on charge current imposed by the
    // availability of power. If we're on solar it's whatever surplus
    // we're generating. If we're on grid power it's infinity.
    let mut max_charge_current: f32 = 0.0;

    let mut varta_state: Option<crate::varta::VartaState> = None;

    loop {
        tokio::select! {
            varta_msg = varta_rx.recv() => {
                // println!("new varta state: {:#?}", varta_rx.borrow());
                let Ok(varta_msg) = varta_msg else {
                    continue;
                };
                let crate::varta::Message::VartaState(new_state) = varta_msg else {
                    continue;
                };
                varta_state = Some(new_state);
            }
            _ = pv_rx.changed() => {
                // println!("new pv state: {:#?}", pv_rx.borrow());
            }
        }

        let pv_state = pv_rx.borrow().clone();

        let (Some(current_varta_state), Some(current_pv_state)) = (&varta_state, &pv_state) else {
            continue;
        };

        //
        // Update the charger command.
        //

        let exporting = current_pv_state.grid_export_watts > 0.0;

        println!("desired charge current: {desired_charge_current:.1} A");

        let (min_soc, max_soc, available_current) = if exporting {
            println!("exporting {} W", current_pv_state.grid_export_watts);
            (
                config.excess_pv_min_soc,
                config.excess_pv_max_soc,
                if current_varta_state.voltage > 0.0 {
                    (current_pv_state.grid_export_watts - config.export_margin_w) / current_varta_state.voltage
                } else {
                    config.default_charge_current
                },
            )
        } else {
            println!("not exporting enough ({} W)", current_pv_state.grid_export_watts);
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

        if current_varta_state.soc >= max_soc {
            command.on = false;
            command.voltage = 0.0;
            command.current = 0.0;
            desired_charge_current = 0.0;
            max_charge_current = 0.0;
        } else if current_varta_state.soc <= min_soc {
            command.on = true;
        }

        command.voltage = current_varta_state.charge_voltage_request;

        if command.on {
            let current_error = current_varta_state.current - current_varta_state.charge_current_request;
            desired_charge_current -= current_error * 0.1;
            desired_charge_current =
                desired_charge_current.clamp(0.0, current_varta_state.charge_current_request);
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
            Some(prev) => &command != prev,
            None => true,
        };

        if is_new {
            last_command = Some(command.clone());
            let _ = command_tx.send(Some(command.clone()));
        }
    }
}

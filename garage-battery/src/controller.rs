use crate::config::ControlConfig;
use crate::types::PvState;

fn do_control_solar(
    config: &crate::config::ControlConfigSolar,
    varta_state: &crate::varta::VartaState,
    pv_state: &PvState,
    charger_command: &mut crate::charger_deltaq_icl1500::ChargerCommand,
) {
    // These are the main parameters of the battery controller. Their
    // values depend on whether we're exporting surplus PV power
    // or not.
    let (min_soc, max_soc, available_current) =
        if pv_state.grid_export_watts > config.export_margin_w {
            println!("exporting {} W", pv_state.grid_export_watts);
            (
                config.excess_pv_min_soc,
                config.excess_pv_max_soc,
                if varta_state.voltage > 0.0 {
                    (pv_state.grid_export_watts - config.export_margin_w) / varta_state.voltage
                } else {
                    // Something's wrong and the Vartas aren't telling
                    // us their terminal voltage, don't charge until
                    // they figure it out.
                    0.0
                },
            )
        } else {
            println!("not exporting enough ({} W)", pv_state.grid_export_watts);
            (
                config.no_excess_min_soc,
                config.no_excess_max_soc,
                f32::INFINITY,
            )
        };

    println!("available current: {:.1} A", available_current);

    if varta_state.soc >= max_soc {
        // Done charging for now, turn off the charger.
        charger_command.on = false;
    } else if varta_state.soc <= min_soc {
        // Battery too low, turn on the charger.
        charger_command.on = true;
    }

    if charger_command.on {
        charger_command.voltage = varta_state.charge_voltage_request;

        let current_error = varta_state.current - varta_state.charge_current_request;
        charger_command.current -= current_error * 0.1;

        charger_command.current = charger_command.current.max(0.0);
        charger_command.current = charger_command
            .current
            .min(varta_state.charge_current_request);
        charger_command.current = charger_command.current.min(available_current);
        charger_command.current = charger_command.current.min(config.charger_max_dc_current);
    } else {
        charger_command.voltage = 0.0;
        charger_command.current = 0.0;
    }
}

fn do_control_hold(
    config: &crate::config::ControlConfigHold,
    varta_state: &crate::varta::VartaState,
    charger_command: &mut crate::charger_deltaq_icl1500::ChargerCommand,
) {
    println!("SoC={:.3} (target={:.3})", varta_state.soc, config.soc);

    charger_command.on = true;
    charger_command.voltage = varta_state.charge_voltage_request;

    let soc_error = varta_state.soc - config.soc;
    let target_current = (soc_error * -250.0).min(varta_state.charge_current_request);
    let current_error = varta_state.current - target_current;

    println!(
        "soc_error={:.3}, target_current={:.3}, varta_current={:.3}, current_error={:.3}",
        soc_error, target_current, varta_state.current, current_error
    );

    charger_command.current -= current_error * 0.1;
    charger_command.current = charger_command.current.max(0.0);
    charger_command.current = charger_command.current.min(config.charger_max_dc_current);
}

pub async fn run(
    config: ControlConfig,
    mut varta_rx: tokio::sync::broadcast::Receiver<crate::varta::Message>,
    mut pv_rx: tokio::sync::watch::Receiver<Option<PvState>>,
    command_tx: tokio::sync::watch::Sender<Option<crate::charger_deltaq_icl1500::ChargerCommand>>,
) {
    let mut charger_command =
        crate::charger_deltaq_icl1500::ChargerCommand { on: false, voltage: 0.0, current: 0.0 };

    // This is how much the batteries are asking for. This gets clipped
    // by the available current before getting sent to the charger.
    // let mut desired_charge_current: f32 = 0.0;

    // This is an upper bound on charge current imposed by the
    // availability of power. If we're on solar it's whatever surplus
    // we're generating. If we're on grid power it's infinity.
    // let mut max_charge_current: f32 = 0.0;

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

        // If we get here then one of our inputs changed and we should
        // re-evaluate the control decision.

        let (Some(current_varta_state), Some(current_pv_state)) =
            (&varta_state, pv_rx.borrow().clone())
        else {
            continue;
        };

        match &config {
            ControlConfig::Solar(control_config_solar) => do_control_solar(
                control_config_solar,
                current_varta_state,
                &current_pv_state,
                &mut charger_command,
            ),
            ControlConfig::Hold(control_config_hold) => do_control_hold(
                control_config_hold,
                current_varta_state,
                &mut charger_command,
            ),
        }

        let _ = command_tx.send(Some(charger_command.clone()));
    }
}

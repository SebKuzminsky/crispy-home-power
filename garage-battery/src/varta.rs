use varta_easyblade::Varta;

/// State from the Varta battery pack.
#[derive(Debug, Clone)]
pub struct VartaState {
    pub soc: f32,
    pub charge_current_request: f32,
    pub charge_voltage_request: f32,
    pub voltage: f32,
    pub current: f32,
}

#[derive(Clone, Debug)]
pub enum Message {
    VartaState(VartaState),
    Msg(String),
}

impl PartialEq for VartaState {
    fn eq(&self, other: &Self) -> bool {
        (self.soc - other.soc).abs() < 0.01
            && (self.charge_current_request - other.charge_current_request).abs() < 0.01
            && (self.charge_voltage_request - other.charge_voltage_request).abs() < 0.01
            && (self.voltage - other.voltage).abs() < 0.01
    }
}

impl Eq for VartaState {}

pub async fn run(can_interface: &str, msg_tx: tokio::sync::broadcast::Sender<Message>) {
    let mut varta: Varta = loop {
        match Varta::new(can_interface).await {
            Ok(v) => break v,
            Err(e) => {
                let _ = msg_tx.send(Message::Msg(format!(
                    "cannot open CAN interface '{}': {}, retrying in 5s...",
                    can_interface, e
                )));
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            },
        }
    };

    let mut last_state: Option<VartaState> = None;

    let mut num_modules = 0;

    loop {
        // FIXME: deal with varta nodes dropping off
        match varta.process_socketcan_msg().await {
            Ok(Some(node_id)) => {
                let _ = msg_tx.send(Message::Msg(format!("discovered module {}", node_id,)));
                num_modules += 1;
            },
            Ok(None) => {},
            Err(e) => {
                let _ = msg_tx.send(Message::Msg(format!("error reading CAN frame: {}", e,)));
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                continue;
            },
        }

        let master = &varta.master;

        // if let (Some(soc), Some(charge_current_request), Some(voltage)) =
        //     (master.soc, master.charge_current_request, master.voltage)
        if let (Some(soc), Some(voltage), Some(current)) =
            (master.soc, master.voltage, master.current)
        {
            let state = VartaState {
                soc,
                charge_current_request: num_modules as f32 * 9.0, // 9.0 A per module
                charge_voltage_request: 59.5,
                voltage,
                current,
            };

            let is_new = match &last_state {
                Some(prev) => &state != prev,
                None => true,
            };

            if is_new {
                last_state = Some(state.clone());
                let _ = msg_tx.send(Message::VartaState(state));
            }
        }
    }
}

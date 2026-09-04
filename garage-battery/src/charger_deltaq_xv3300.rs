/// Command to the Delta-Q charger.
#[derive(Debug, Clone)]
pub struct ChargerCommand {
    pub can_listen_only: bool,
    pub max_ac_current: f32,
}

// impl PartialEq for ChargerCommand {
//     fn eq(&self, other: &Self) -> bool {
//         self.on == other.on
//             && (self.voltage - other.voltage).abs() < 0.01
//             && (self.current - other.current).abs() < 0.01
//     }
// }

pub async fn run(
    can_interface: &str,
    command_interval_secs: f64,
    mut command_rx: tokio::sync::watch::Receiver<Option<ChargerCommand>>,
) {
    let (tx, rx) = zencan_client::open_socketcan(can_interface).unwrap();
    let mut sdo_client = zencan_client::SdoClient::new_std(0x64, tx, rx);

    let interval = tokio::time::Duration::from_secs_f64(command_interval_secs);
    let timer = tokio::time::sleep(interval);
    tokio::pin!(timer);

    loop {
        tokio::select! {
            _ = command_rx.changed() => {}
            _ = &mut timer => {}
        }

        let command = command_rx.borrow().clone();
        // println!("charge command: {command:#?}");

        if let Some(ChargerCommand { can_listen_only, max_ac_current }) = command {
            match deltaq_xv3300::set_can_listen_only(&mut sdo_client, can_listen_only).await {
                Ok(()) => {
                    if can_listen_only {
                        println!("deltaq-xv3300 charger: CAN bus is listen-only, no charging");
                    } else {
                        println!("deltaq-xv3300 charger: CAN bus is active, charging available");
                    }
                },
                Err(e) => {
                    println!("deltaq-xv3300 charger: SDO error: {}", e);
                },
            }

            match deltaq_xv3300::set_ac_current_limit(&mut sdo_client, max_ac_current).await {
                Ok(()) => {
                    println!("deltaq-xv3300 charger: AC current limit {max_ac_current:.3} A");
                },
                Err(e) => {
                    println!("deltaq-xv3300 charger: SDO error: {}", e);
                },
            }
        }

        timer.set(tokio::time::sleep(interval));
    }
}

pub async fn shutdown(can_interface: &str) {
    println!("deltaq-xv3300 charger: shutting down");

    let (tx, rx) = zencan_client::open_socketcan(can_interface).unwrap();
    let mut sdo_client = zencan_client::SdoClient::new_std(0x64, tx, rx);

    match deltaq_xv3300::set_can_listen_only(&mut sdo_client, false).await {
        Ok(()) => {
            println!("deltaq-xv3300 charger: CAN bus is active");
        },
        Err(e) => {
            println!("deltaq-xv3300 charger: SDO error: {}", e);
        },
    }

    match deltaq_xv3300::set_ac_current_limit(&mut sdo_client, 10.0).await {
        Ok(()) => {
            println!("deltaq-xv3300 charger: AC current limit 10 A");
        },
        Err(e) => {
            println!("deltaq-xv3300 charger: SDO error: {}", e);
        },
    }
}

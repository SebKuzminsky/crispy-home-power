use crate::types::ChargerCommand;

pub async fn run(
    can_interface: &str,
    command_interval_secs: f64,
    mut command_rx: tokio::sync::watch::Receiver<Option<ChargerCommand>>,
) {
    let can_socket: tokio_socketcan::CANSocket = loop {
        match tokio_socketcan::CANSocket::open(can_interface) {
            Ok(s) => break s,
            Err(e) => {
                println!(
                    "charger: cannot open CAN interface '{}': {}, retrying in 5s...",
                    can_interface, e
                );
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            },
        }
    };

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

        if let Some(cmd) = command {
            match charger::send_command(&can_socket, cmd.voltage, cmd.current, 20.0, 0).await {
                Ok(()) => {
                    if cmd.on {
                        println!(
                            "charger: charging voltage={:.3}V current={:.3}A",
                            cmd.voltage, cmd.current
                        );
                    } else {
                        println!("charger: OFF");
                    }
                },
                Err(e) => {
                    println!("charger: send_command error: {}", e);
                },
            }
        }

        timer.set(tokio::time::sleep(interval));
    }
}

pub async fn shutdown(can_interface: &str) {
    match tokio_socketcan::CANSocket::open(can_interface) {
        Ok(socket) => {
            let _ = charger::send_command(&socket, 0.0, 0.0, 20.0, 0).await;
            println!("charger: shutdown command sent");
        },
        Err(e) => {
            println!("charger: cannot open CAN for shutdown: {}", e);
        },
    }
}

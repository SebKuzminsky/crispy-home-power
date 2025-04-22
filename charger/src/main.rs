use embedded_can::Frame;
use futures_util::stream::StreamExt;

use clap::Parser;

mod delta_q_can_messages;

/// Command voltage and current from a DeltaQ ICL 1500-058 charger.
#[derive(clap::Parser, Debug)]
#[command(version, about, long_about=None)]
struct Args {
    #[arg(long, short = 'v')]
    volts: f32,

    #[arg(long, short = 'a')]
    amps: f32,

    #[arg(long, short = 't', default_value_t = 20.0)]
    temperature: f32,

    #[arg(long, short = 's', default_value_t = 50)]
    soc: u8,

    #[arg(long, short = 'c', default_value_t = String::from("can0"))]
    can_interface: String,
}

#[allow(dead_code)]
fn handle_can_frame(frame: tokio_socketcan::CANFrame) -> Result<(), eyre::Report> {
    let id: embedded_can::Id = if frame.is_extended() {
        match embedded_can::ExtendedId::new(frame.id()) {
            Some(id) => embedded_can::Id::Extended(id),
            None => return Err(eyre::eyre!("invalid extended frame id {}", frame.id())),
        }
    } else {
        match embedded_can::StandardId::new(frame.id() as u16) {
            Some(id) => embedded_can::Id::Standard(id),
            None => return Err(eyre::eyre!("invalid standard frame id {}", frame.id())),
        }
    };

    let msg = delta_q_can_messages::Messages::from_can_message(id, frame.data());
    println!("{:#?}", msg);

    Ok(())
}

async fn send_command(
    can_socket_tx: &tokio_socketcan::CANSocket,
    volts: f32,
    amps: f32,
    temperature: f32,
    soc: u8,
) -> Result<(), eyre::Report> {
    let frame = delta_q_can_messages::DeltaQRpdo20x30a::new(amps, volts, temperature)?;
    let id: u32 = match frame.id() {
        embedded_can::Id::Standard(standard_id) => standard_id.as_raw() as u32,
        embedded_can::Id::Extended(extended_id) => extended_id.as_raw(),
    };
    let raw_frame = tokio_socketcan::CANFrame::new(id, frame.raw(), false, false)?;
    can_socket_tx.write_frame(raw_frame)?.await?;

    let batt_charge_cycle_time = match amps {
        0.0 => delta_q_can_messages::DeltaQRpdo10x20aBattChargeCycleType::NoActiveCycle,
        _ => delta_q_can_messages::DeltaQRpdo10x20aBattChargeCycleType::Charge,
    };

    let battery_status = match amps {
        0.0 => delta_q_can_messages::DeltaQRpdo10x20aBatteryStatus::Disabled,
        _ => delta_q_can_messages::DeltaQRpdo10x20aBatteryStatus::Enabled,
    };

    let frame = delta_q_can_messages::DeltaQRpdo10x20a::new(
        soc,
        batt_charge_cycle_time.into(),
        volts,
        amps,
        battery_status.into(),
    )?;
    let id: u32 = match frame.id() {
        embedded_can::Id::Standard(standard_id) => standard_id.as_raw() as u32,
        embedded_can::Id::Extended(extended_id) => extended_id.as_raw(),
    };
    let raw_frame = tokio_socketcan::CANFrame::new(id, frame.raw(), false, false)?;
    can_socket_tx.write_frame(raw_frame)?.await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), eyre::Report> {
    let args = Args::parse();
    println!("config: {args:#?}");

    let mut can_socket_rx = tokio_socketcan::CANSocket::open(&args.can_interface)?;
    let can_socket_tx = tokio_socketcan::CANSocket::open(&args.can_interface)?;

    let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(1));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            maybe_frame = can_socket_rx.next() => {
                match maybe_frame {
                    Some(Ok(_frame)) => {
                        // let _ = handle_can_frame(frame);
                    }
                    _ => ()
                }
            }
            _ = &mut timeout => {
                let _ = send_command(&can_socket_tx, args.volts, args.amps, args.temperature, args.soc).await;
                timeout.set(tokio::time::sleep(tokio::time::Duration::from_secs(1)));
            }
        }
    }
}

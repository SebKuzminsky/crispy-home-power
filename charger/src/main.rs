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
    args: &Args,
) -> Result<(), eyre::Report> {
    let frame = delta_q_can_messages::DeltaQRpdo20x30a::new(args.amps, args.volts, 0.0)?;
    let id: u32 = match frame.id() {
        embedded_can::Id::Standard(standard_id) => standard_id.as_raw() as u32,
        embedded_can::Id::Extended(extended_id) => extended_id.as_raw(),
    };
    let raw_frame = tokio_socketcan::CANFrame::new(id, frame.raw(), false, false)?;
    can_socket_tx.write_frame(raw_frame)?.await?;

    // cansend can0 '20a#00.31.01.00.32.20.00.01' ; sleep 1 ; done`
    let frame = delta_q_can_messages::DeltaQRpdo10x20a::new(
        50,
        delta_q_can_messages::DeltaQRpdo10x20aBattChargeCycleType::Charge.into(),
        args.volts,
        args.amps,
        delta_q_can_messages::DeltaQRpdo10x20aBatteryStatus::Enabled.into(),
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
                let _ = send_command(&can_socket_tx, &args).await;
                timeout.set(tokio::time::sleep(tokio::time::Duration::from_secs(1)));
            }
        }
    }
}

mod charger;
mod config;
mod controller;
mod pv_monitor;
mod types;
mod varta;

use clap::Parser;

#[derive(clap::Parser, Debug)]
#[command(version, about = "Garage battery management system")]
struct Args {
    #[arg(long, short = 'c', default_value = "config.toml")]
    config: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let cfg = config::load(&args.config);

    let auth_token = match std::fs::read_to_string(&cfg.envoy_auth_token_path) {
        Ok(t) => t.trim().to_string(),
        Err(e) => {
            println!(
                "cannot read auth token from '{}': {}",
                cfg.envoy_auth_token_path, e
            );
            std::process::exit(1);
        },
    };

    let (varta_tx, mut varta_rx) = tokio::sync::broadcast::channel::<varta::Message>(100);
    let varta_task = {
        let varta_can = cfg.varta_can_interface.clone();
        let varta_tx = varta_tx.clone();
        tokio::spawn(async move {
            varta::run(&varta_can, varta_tx).await;
        })
    };

    let (pv_tx, pv_rx) = tokio::sync::watch::channel::<Option<types::PvState>>(None);
    let pv_task = {
        let envoy_url = cfg.envoy_url.clone();
        tokio::spawn(async move {
            pv_monitor::run(&envoy_url, &auth_token, cfg.pv_poll_interval_secs, pv_tx).await;
        })
    };

    let (charger_cmd_tx, charger_cmd_rx) =
        tokio::sync::watch::channel::<Option<charger::ChargerCommand>>(None);
    let charger_task = {
        let charger_can = cfg.charger_can_interface.clone();
        tokio::spawn(async move {
            charger::run(
                &charger_can,
                cfg.charger_command_interval_secs,
                charger_cmd_rx,
            )
            .await;
        })
    };

    let controller_task = {
        let control_config = cfg.control.clone();
        let varta_rx = varta_tx.subscribe();
        tokio::spawn(async move {
            controller::run(control_config, varta_rx, pv_rx, charger_cmd_tx).await;
        })
    };

    loop {
        tokio::select! {
            // Wait for the user to hit Ctrl-C
            r = tokio::signal::ctrl_c() => {
                if let Err(e) = r {
                    panic!("failed to listen for ctrl-c: {e:?}");
                }
                println!("received Ctrl-C, shutting down");
                break;
            }

            varta_msg = varta_rx.recv() => {
                let Ok(varta_msg) = varta_msg else {
                    println!("error reading varta msg: {varta_msg:?}");
                    continue;
                };
                match varta_msg {
                    crate::varta::Message::Msg(msg) => println!("varta: {}", msg),
                    _ => {},
                }
            }
        }
    }

    varta_task.abort();
    pv_task.abort();
    controller_task.abort();

    charger_task.abort();
    charger::shutdown(&cfg.charger_can_interface).await;

    println!("shutdown: complete");
}

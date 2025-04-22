use clap::Parser;

mod app;
mod tui;

/// Read telemetry from ABS Alliance E48-2.0 batteries.
#[derive(clap::Parser, Debug)]
#[command(version, about, long_about=None)]
struct Args {
    #[arg(long, short = 'c', default_value_t = String::from("can0"))]
    can_interface: String,
}

#[tokio::main]
async fn main() -> Result<(), eyre::Report> {
    let args = Args::parse();
    println!("config: {args:#?}");

    let mut app = app::App::new(&args.can_interface)?;

    let terminal = tui::init()?;
    let result = app.run(terminal).await;
    let _ = tui::restore();
    result
}

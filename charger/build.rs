use dbc_codegen::{Config, FeatureConfig};

use anyhow::{Context, Result};

fn main() -> Result<()> {
    let dbc_path = String::from("./delta_q.dbc");
    let dbc_file = std::fs::read(&dbc_path).context("failed to read DBC file {dbc_path}\n")?;
    println!("cargo:rerun-if-changed={}", &dbc_path);

    let config = Config::builder()
        .dbc_name(&dbc_path)
        .dbc_content(&dbc_file)
        .allow_dead_code(true) // Don't emit warnings if not all generated code is used
        //.impl_arbitrary(FeatureConfig::Gated("arbitrary")) // Optional impls.
        .impl_debug(FeatureConfig::Always) // See rustdoc for more,
        .impl_error(FeatureConfig::Gated("std"))
        //.check_ranges(FeatureConfig::Never)                // or look below for an example.
        .build();

    let messages_path = String::from("src/delta_q_can_messages.rs");
    if let Err(e) = std::fs::remove_file(&messages_path) {
        println!("Failed to remove {messages_path}: {e:?}");
        println!("oh well");
    }
    let mut out = std::io::BufWriter::new(std::fs::File::create(&messages_path).unwrap());
    dbc_codegen::codegen(config, &mut out).context("dbc-codegen failed")?;
    Ok(())
}

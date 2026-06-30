use anyhow::Context;

fn main() {
    let dbc_path = String::from("./powertrain_multimod_v78.00.007.dbc");
    let dbc_contents = std::fs::read_to_string(&dbc_path)
        .context("failed to read DBC file {dbc_path}\n")
        .unwrap();
    println!("cargo:rerun-if-changed={}", &dbc_path);

    let output_path = String::from("src/abs_alliance_can_messages.rs");

    dbc_codegen::Config::builder()
        .dbc_name(&dbc_path)
        .dbc_content(&dbc_contents)
        .allow_dead_code(true) // Don't emit warnings if not all generated code is used
        //.impl_arbitrary(dbc_codegen::FeatureConfig::Gated("arbitrary")) // Optional impls.
        .impl_debug(dbc_codegen::FeatureConfig::Always) // See rustdoc for more,
        .impl_error(dbc_codegen::FeatureConfig::Gated("std"))
        //.check_ranges(dbc_codegen::FeatureConfig::Never)                // or look below for an example.
        .build()
        .write_to_file(&output_path)
        .unwrap();
}

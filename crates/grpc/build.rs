fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_dir = format!(
        "{}/../../proto",
        std::env::var("CARGO_MANIFEST_DIR").unwrap()
    );
    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .compile_protos(&["zero/api/v1/control.proto"], &[&proto_dir])?;
    Ok(())
}

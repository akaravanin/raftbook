fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(false) // no client needed in the server binary
        .compile(
            &["../../proto/exchange.proto"],
            &["../../proto"],
        )?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure().compile_protos(
        &[
            "../proto/collector.proto",
            "../proto/indexer.proto",
            "../proto/scheduler.proto",
        ],
        &["../proto"],
    )?;

    Ok(())
}

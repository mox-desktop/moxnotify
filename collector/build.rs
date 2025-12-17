fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .compile_protos(
            &[
                "../proto/common.proto",
                "../proto/types.proto",
                "../proto/collector.proto",
            ],
            &["../proto"],
        )?;
    Ok(())
}

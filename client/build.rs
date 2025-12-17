fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .type_attribute(".", "#[serde(rename_all = \"snake_case\")]")
        .compile_protos(
            &[
                "../proto/common.proto",
                "../proto/types.proto",
                "../proto/client.proto",
            ],
            &["../proto"],
        )?;

    Ok(())
}

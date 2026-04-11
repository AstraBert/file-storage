use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all("src/grpc")?;
    tonic_prost_build::configure()
        .out_dir("src/grpc")
        .compile_protos(
            &["../proto-definitions/file_storage.proto"],
            &["../proto-definitions"],
        )?;
    Ok(())
}

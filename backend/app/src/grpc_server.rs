use std::env;
use std::sync::Arc;

use aws_sdk_s3::config::{Credentials, Region};
use aws_sdk_s3::{Client, Config};
use proto::FileStorageServer;
use proto::grpc::file_storage::file_storage_service_server::FileStorageServiceServer;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "0.0.0.0:50051".parse()?;
    let garage_access_key_id =
        env::var("GARAGE_KEY_ID").expect("GARAGE_KEY_ID should be defined in the environment");
    let garage_secret_key = env::var("GARAGE_SECRET_KEY")
        .expect("GARAGE_SECRET_KEY should be defined in the environment");
    let credentials = Credentials::new(
        garage_access_key_id,
        garage_secret_key,
        None,
        None,
        "garage",
    );

    let config = Config::builder()
        .credentials_provider(credentials)
        .behavior_version_latest()
        .region(Region::new("garage"))
        .endpoint_url("http://garage:3900")
        .force_path_style(true)
        .build();

    let aws_client = Client::from_conf(config);
    println!("Created S3 client connected to Garage");
    let server = FileStorageServer {
        client: Arc::new(aws_client),
    };

    println!("Starting to serve GRPC server on port 50051");
    Server::builder()
        .add_service(FileStorageServiceServer::new(server))
        .serve(addr)
        .await?;

    Ok(())
}

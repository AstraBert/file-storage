use anyhow::Result;

use crate::serving::RagServer;

mod chunking;
mod embeddings;
mod serving;
mod vectordb;

const QDRANT_URL: &str = "http://qdrant:6334";
const COLLECTION_NAME: &str = "file_storage_search";

#[tokio::main]
async fn main() -> Result<()> {
    let server = RagServer::new(
        QDRANT_URL.to_string(),
        COLLECTION_NAME.to_string(),
        None,
        None,
        Some(100_u32),
    );
    server.serve().await?;
    Ok(())
}

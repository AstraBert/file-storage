use crate::{chunking::chunk_text, embeddings::embed_chunks, vectordb::VectorDB};

#[derive(Debug)]
pub struct Pipeline {
    // Chunking options
    pub chunk_size: usize,
    // VectorDB options
    qdrant_url: String,
    pub collection_name: String,
}

impl Pipeline {
    pub fn new(chunk_size: usize, qdrant_url: String, collection_name: String) -> Self {
        Self {
            chunk_size,
            qdrant_url,
            collection_name,
        }
    }

    #[tracing::instrument]
    pub async fn run(&self, input_text: &str, user_identifier: &str) -> anyhow::Result<()> {
        let vectordb = VectorDB::new(self.qdrant_url.clone(), self.collection_name.clone());
        vectordb.create_collection().await?;
        tracing::debug!(
            event = "ingestion_pipeline_run",
            status = "started",
            progress = "ensured_collection"
        );
        let mut chunks = chunk_text(input_text, self.chunk_size);
        tracing::debug!(
            event = "ingestion_pipeline_run",
            status = "started",
            progress = "chunked_text"
        );
        chunks = embed_chunks(chunks);
        tracing::debug!(
            event = "ingestion_pipeline_run",
            status = "started",
            progress = "embedded_chunks"
        );
        vectordb.upload_embeddings(chunks, user_identifier).await?;
        tracing::debug!(
            event = "ingestion_pipeline_run",
            status = "completed",
            progress = "uploaded_embeddings"
        );
        Ok(())
    }
}

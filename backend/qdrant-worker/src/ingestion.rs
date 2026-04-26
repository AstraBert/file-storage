use std::sync::Arc;

use utils::{STATUS_COMPLETED, STATUS_STARTED};

use crate::{chunking::chunk_text, embeddings::embed_chunks, vectordb::VectorDB};

#[derive(Debug)]
pub struct Pipeline {
    // Chunking options
    pub chunk_size: usize,
    vectordb: Arc<VectorDB>,
}

impl Pipeline {
    pub fn new(
        chunk_size: usize,
        qdrant_url: String,
        collection_name: String,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            chunk_size,
            vectordb: Arc::new(VectorDB::new(qdrant_url, collection_name)?),
        })
    }

    #[tracing::instrument]
    pub async fn run(&self, input_text: &str, user_identifier: &str) -> anyhow::Result<()> {
        self.vectordb.create_collection().await?;
        tracing::info!(
            event = "ingestion_pipeline_run",
            status = STATUS_STARTED,
            progress = "ensured_collection"
        );
        let mut chunks = chunk_text(input_text, self.chunk_size);
        tracing::info!(
            event = "ingestion_pipeline_run",
            status = STATUS_STARTED,
            progress = "chunked_text"
        );
        chunks = embed_chunks(chunks);
        tracing::info!(
            event = "ingestion_pipeline_run",
            status = STATUS_STARTED,
            progress = "embedded_chunks"
        );
        self.vectordb
            .upload_embeddings(chunks, user_identifier)
            .await?;
        tracing::info!(
            event = "ingestion_pipeline_run",
            status = STATUS_COMPLETED,
            progress = "uploaded_embeddings"
        );
        Ok(())
    }

    #[tracing::instrument]
    pub async fn delete(&self, content: &str, user_identifier: &str) -> anyhow::Result<()> {
        tracing::info!(event = "ingestion_pipeline_delete", status = STATUS_STARTED);
        self.vectordb.delete_point(content, user_identifier).await?;
        tracing::info!(
            event = "ingestion_pipeline_delete",
            status = STATUS_COMPLETED
        );
        log::debug!("Successfully deleted point from Qdrant collection");
        Ok(())
    }
}

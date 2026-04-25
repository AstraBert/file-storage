use crate::{chunking::chunk_text, embeddings::embed_chunks, vectordb::VectorDB};

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

    pub async fn run(&self, input_text: &str, user_identifier: &str) -> anyhow::Result<()> {
        let vectordb = VectorDB::new(self.qdrant_url.clone(), self.collection_name.clone());
        vectordb.create_collection().await?;
        let mut chunks = chunk_text(input_text, self.chunk_size);
        chunks = embed_chunks(chunks);
        vectordb.upload_embeddings(chunks, user_identifier).await?;
        Ok(())
    }
}

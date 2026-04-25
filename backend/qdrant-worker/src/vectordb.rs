use bm25::Embedding;
use qdrant_client::{
    Payload, Qdrant,
    qdrant::{
        Condition, CreateCollectionBuilder, Filter, NamedVectors, PointStruct, QueryPointsBuilder,
        SparseVectorParamsBuilder, SparseVectorsConfigBuilder, UpsertPointsBuilder, Vector,
    },
};
use std::collections::HashMap;

use crate::chunking::Chunk;

#[derive(Debug, Clone)]
pub struct VectorDB {
    pub collection_name: String,
    pub url: String,
}

impl VectorDB {
    pub fn new(url: String, collection_name: String) -> Self {
        Self {
            collection_name,
            url,
        }
    }

    pub async fn create_collection(&self) -> anyhow::Result<()> {
        let client = Qdrant::from_url(&self.url)
            .api_key(std::env::var("QDRANT_API_KEY"))
            .build()?;
        println!("Starting to create collection {}", self.collection_name);
        let collection_exists = client.collection_exists(&self.collection_name).await?;
        if collection_exists {
            println!("Collection {} already exists", self.collection_name);
            return Ok(());
        }
        let mut sparse_vector_config = SparseVectorsConfigBuilder::default();
        sparse_vector_config.add_named_vector_params("text", SparseVectorParamsBuilder::default());
        let response = client
            .create_collection(
                CreateCollectionBuilder::new(&self.collection_name)
                    .sparse_vectors_config(sparse_vector_config),
            )
            .await?;
        if response.result {
            println!("Collection {} successfully created", self.collection_name);
            Ok(())
        } else {
            eprintln!(
                "There was an error creating collection: {}",
                self.collection_name
            );
            Err(anyhow::anyhow!(
                "There was an error creating the Qdrant collection"
            ))
        }
    }

    pub async fn upload_embeddings(
        &self,
        chunks: Vec<Chunk>,
        user_identifier: &str,
    ) -> anyhow::Result<()> {
        let collection_ready = self.check_collection_ready().await;
        let mut base_id = match collection_ready {
            Ok(num_points) => {
                // not ready -> exists but does not contain points
                if num_points == 0 {
                    num_points
                } else {
                    // ready -> exists and contains points
                    log::warn!(
                        "WARNING: collection already has {:?} points, preparing to upload more...",
                        num_points
                    );
                    num_points
                }
            }
            // error: does not exist or fails to check for points
            Err(e) => {
                log::error!(
                    "There was an error during the collection health check: {}",
                    e,
                );
                return Err(anyhow::anyhow!(
                    "There was an error during the collection health check"
                ));
            }
        };
        let client = Qdrant::from_url(&self.url)
            .api_key(std::env::var("QDRANT_API_KEY"))
            .build()?;
        log::debug!(
            "Starting to upload embeddings to collection {}",
            self.collection_name
        );
        let collection_exists = client.collection_exists(&self.collection_name).await?;
        if !collection_exists {
            log::error!(
                "Collection {} does not exist. Please run `create_collection` before using this function",
                self.collection_name
            );
            return Err(anyhow::anyhow!(
                "Collection does not exist. Please run `create_collection` before using this function"
            ));
        }
        let mut points: Vec<PointStruct> = vec![];
        for chunk in chunks {
            base_id += 1;
            let embd = match chunk.embedding {
                Some(e) => e,
                None => {
                    log::warn!(
                        "Embedding {:?} does not have an associated embedding, skipping...",
                        base_id
                    );
                    continue;
                }
            };
            let mut index_map: HashMap<u32, f32> = HashMap::new();
            for token in &embd.0 {
                *index_map.entry(token.index).or_insert(0.0) += token.value;
            }
            let mut index_value_pairs: Vec<_> = index_map.into_iter().collect();
            index_value_pairs.sort_by_key(|(idx, _)| *idx);
            let (indices, values): (Vec<u32>, Vec<f32>) = index_value_pairs.into_iter().unzip();
            let vector = Vector::new_sparse(indices, values);
            let mut payload = Payload::new();
            payload.insert("content", chunk.content);
            payload.insert("user_identifier", user_identifier);
            let point = PointStruct::new(
                base_id,
                NamedVectors::default().add_vector("text", vector),
                payload,
            );
            points.push(point);
        }
        let response = client
            .upsert_points(UpsertPointsBuilder::new(&self.collection_name, points))
            .await?;
        match response.result {
            Some(_) => {
                println!("All the vectors have been succcessfully uploaded");
            }
            None => {
                eprintln!("The uploading operation did not produce any result");
                return Err(anyhow::anyhow!(
                    "The uploading operation did not produce any result"
                ));
            }
        }
        Ok(())
    }

    pub async fn check_collection_ready(&self) -> anyhow::Result<u64> {
        let client = Qdrant::from_url(&self.url)
            .api_key(std::env::var("QDRANT_API_KEY"))
            .build()?;
        let collection_exists = client.collection_exists(&self.collection_name).await?;
        if !collection_exists {
            eprintln!(
                "Collection {} does not exist. Please run `create_collection` before using this function",
                self.collection_name
            );
            return Err(anyhow::anyhow!(
                "Collection does not exist. Please run `create_collection` before using this function"
            ));
        }
        let result = client.collection_info(&self.collection_name).await?;
        let collection_info = match result.result {
            Some(r) => r,
            None => {
                eprintln!("Could not retrieve collection information");
                return Err(anyhow::anyhow!("Could not retrieve collection information"));
            }
        };
        match collection_info.points_count {
            Some(p) => {
                if p > 0 {
                    Ok(p)
                } else {
                    Ok(0_u64)
                }
            }
            None => {
                eprintln!("Could not retrieve the number of data points in the collection");
                Err(anyhow::anyhow!(
                    "Could not retrieve the number of data points in the collection"
                ))
            }
        }
    }

    pub async fn search(
        self,
        embedding: Embedding,
        limit: u64,
        user_identifier: &str,
    ) -> anyhow::Result<Vec<String>> {
        let client = Qdrant::from_url(&self.url)
            .api_key(std::env::var("QDRANT_API_KEY"))
            .build()?;
        let mut indices_values: Vec<(u32, f32)> = vec![];
        for token in &embedding.0 {
            indices_values.push((token.index, token.value));
        }
        let query = QueryPointsBuilder::new(&self.collection_name)
            .query(indices_values)
            .limit(limit)
            .filter(Filter::must([Condition::matches(
                "user_identifier",
                user_identifier.to_string(),
            )]))
            .with_payload(true)
            .using("text");
        let results = client.query(query).await?;
        let mut contents: Vec<String> = vec![];
        for res in results.result {
            if res.payload.contains_key("content") {
                let content: String = match res.payload.get("content") {
                    Some(s) => s.to_string(),
                    None => {
                        eprintln!("Could not retrieve content, skipping...");
                        continue;
                    }
                };
                contents.push(content);
            } else {
                eprintln!("Point does not have an associated text content");
            }
        }

        Ok(contents)
    }
}

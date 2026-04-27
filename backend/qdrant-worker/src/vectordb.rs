use bm25::Embedding;
use qdrant_client::qdrant::point_id::PointIdOptions::{Num, Uuid};
use qdrant_client::{
    Payload, Qdrant,
    qdrant::{
        Condition, CountPointsBuilder, CreateCollectionBuilder, DeletePointsBuilder, Filter,
        NamedVectors, PointStruct, PointsIdsList, QueryPointsBuilder, ScrollPointsBuilder,
        SparseVectorParamsBuilder, SparseVectorsConfigBuilder, UpsertPointsBuilder, Vector,
    },
};
use std::{collections::HashMap, fmt, sync::Arc};

use crate::chunking::Chunk;

#[derive(Clone)]
pub struct DebuggableQdrant(pub Arc<Qdrant>);

impl fmt::Debug for DebuggableQdrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Qdrant")
    }
}

#[derive(Debug, Clone)]
pub struct VectorDB {
    client: DebuggableQdrant,
    collection_name: String,
}

impl VectorDB {
    pub fn new(url: String, collection_name: String) -> anyhow::Result<Self> {
        let client = Qdrant::from_url(&url)
            .api_key(std::env::var("QDRANT_API_KEY"))
            .build()?;
        Ok(Self {
            client: DebuggableQdrant(Arc::new(client)),
            collection_name: collection_name,
        })
    }

    pub async fn create_collection(&self) -> anyhow::Result<()> {
        log::debug!("Starting to create collection {}", self.collection_name);
        let collection_exists = self
            .client
            .0
            .collection_exists(&self.collection_name)
            .await?;
        if collection_exists {
            log::debug!("Collection {} already exists", self.collection_name);
            return Ok(());
        }
        let mut sparse_vector_config = SparseVectorsConfigBuilder::default();
        sparse_vector_config.add_named_vector_params("text", SparseVectorParamsBuilder::default());
        let response = self
            .client
            .0
            .create_collection(
                CreateCollectionBuilder::new(&self.collection_name)
                    .sparse_vectors_config(sparse_vector_config),
            )
            .await?;
        if response.result {
            log::debug!("Collection {} successfully created", self.collection_name);
            Ok(())
        } else {
            log::error!(
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
                        "WARNING: collection already has points with ID up to {:?}, preparing to upload more...",
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
        log::debug!(
            "Starting to upload embeddings to collection {}",
            self.collection_name
        );
        let collection_exists = self
            .client
            .0
            .collection_exists(&self.collection_name)
            .await?;
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
        let response = self
            .client
            .0
            .upsert_points(UpsertPointsBuilder::new(&self.collection_name, points))
            .await?;
        match response.result {
            Some(_) => {
                log::debug!("All the vectors have been succcessfully uploaded");
            }
            None => {
                log::error!("The uploading operation did not produce any result");
                return Err(anyhow::anyhow!(
                    "The uploading operation did not produce any result"
                ));
            }
        }
        Ok(())
    }

    pub async fn check_collection_ready(&self) -> anyhow::Result<u64> {
        let collection_exists = self
            .client
            .0
            .collection_exists(&self.collection_name)
            .await?;
        if !collection_exists {
            log::warn!(
                "Collection {} does not exist. Creating it now...",
                self.collection_name
            );
            self.create_collection().await?;
        }
        let points_count = self
            .client
            .0
            .count(CountPointsBuilder::new(&self.collection_name).exact(true))
            .await?;
        let points_number = points_count.result.unwrap_or_default().count;
        if points_number == 0 {
            return Ok(points_number);
        }
        let all_points = self
            .client
            .0
            .scroll(ScrollPointsBuilder::new(&self.collection_name).limit(points_number as u32))
            .await?;
        let mut ids = Vec::with_capacity(all_points.result.len());
        for point in all_points.result {
            if let Some(point_id) = point.id {
                if let Some(point_options) = point_id.point_id_options {
                    match point_options {
                        Num(i) => ids.push(i),
                        Uuid(_) => {
                            return Err(anyhow::anyhow!(
                                "Found a point carrying a UUID as ID instead of a u64"
                            ));
                        }
                    }
                }
            }
        }
        if !ids.is_empty() {
            ids.sort();
            return Ok(*ids.last().unwrap());
        }
        Ok(0_u64)
    }

    pub async fn search(
        &self,
        embedding: Embedding,
        limit: u64,
        user_identifier: &str,
    ) -> anyhow::Result<Vec<String>> {
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
        let results = self.client.0.query(query).await?;
        let mut contents: Vec<String> = vec![];
        for res in results.result {
            if res.payload.contains_key("content") {
                let content: String = match res.payload.get("content") {
                    Some(s) => s.to_string(),
                    None => {
                        log::error!("Could not retrieve content, skipping...");
                        continue;
                    }
                };
                contents.push(content);
            } else {
                log::error!("Point does not have an associated text content");
            }
        }

        Ok(contents)
    }

    pub async fn delete_point(&self, content: &str, user_identifier: &str) -> anyhow::Result<()> {
        let response = self
            .client
            .0
            .scroll(
                ScrollPointsBuilder::new(&self.collection_name)
                    .filter(Filter::must([
                        Condition::matches("content", content.to_string()),
                        Condition::matches("user_identifier", user_identifier.to_string()),
                    ]))
                    .limit(1),
            )
            .await?;
        if response.result.is_empty() {
            // point does not exist
            return Ok(());
        }
        if let Some(first) = response.result.first() {
            self.client
                .0
                .delete_points(
                    DeletePointsBuilder::new(&self.collection_name)
                        .points(PointsIdsList {
                            ids: vec![first.id.clone().unwrap()],
                        })
                        .wait(true),
                )
                .await?;
        }

        Ok(())
    }
}

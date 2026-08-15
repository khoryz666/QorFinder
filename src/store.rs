use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use qdrant_client::Qdrant;
use qdrant_client::qdrant::value::Kind;
use qdrant_client::qdrant::{
    Condition, CountPointsBuilder, CreateCollectionBuilder, DeletePointsBuilder, Distance, Filter,
    PointId, PointStruct, ScoredPoint, SearchPointsBuilder, UpsertPointsBuilder, Value,
    VectorParamsBuilder, points_selector::PointsSelectorOneOf,
};
use uuid::Uuid;

/// Qdrant's default gRPC port (the Rust client speaks gRPC; REST is 6333).
pub const DEFAULT_URL: &str = "http://localhost:6334";
pub const DEFAULT_COLLECTION: &str = "qorfinder";

pub struct SearchHit {
    pub file_path: String,
    pub chunk_index: u64,
    pub score: f32,
    pub text: String,
}

pub struct Store {
    client: Qdrant,
    collection: String,
}

impl Store {
    /// Connect to Qdrant and ensure the collection exists with the given
    /// vector size. Errors out if an existing collection has different dims.
    pub async fn connect(url: &str, collection: &str, dims: usize) -> Result<Self> {
        let client = Qdrant::from_url(url)
            .skip_compatibility_check()
            .build()
            .with_context(|| format!("failed to connect to Qdrant at {url}"))?;
        let store = Self {
            client,
            collection: collection.to_string(),
        };
        store.ensure_collection(dims).await?;
        Ok(store)
    }

    async fn ensure_collection(&self, dims: usize) -> Result<()> {
        if !self.client.collection_exists(&self.collection).await? {
            self.client
                .create_collection(
                    CreateCollectionBuilder::new(&self.collection)
                        .vectors_config(VectorParamsBuilder::new(dims as u64, Distance::Cosine))
                        .build(),
                )
                .await
                .with_context(|| format!("failed to create collection '{}'", self.collection))?;
            tracing::info!(
                "created collection '{}' ({} dims, cosine)",
                self.collection,
                dims
            );
            return Ok(());
        }
        if let Some(info) = self.client.collection_info(&self.collection).await?.result {
            let actual = info
                .config
                .and_then(|c| c.params)
                .and_then(|p| p.vectors_config)
                .and_then(|v| v.config)
                .and_then(|c| match c {
                    qdrant_client::qdrant::vectors_config::Config::Params(p) => {
                        Some(p.size as usize)
                    }
                    _ => None,
                });
            if let Some(actual) = actual {
                anyhow::ensure!(
                    actual == dims,
                    "collection '{}' has {} dims but the embedding model produces {}; \
                     recreate the collection or use a matching model",
                    self.collection,
                    actual,
                    dims
                );
            }
        }
        Ok(())
    }

    /// Replace all points belonging to `path` with one point per chunk.
    /// Point IDs are deterministic (UUID v5 of path + chunk index), so
    /// re-indexing the same file is idempotent.
    pub async fn upsert_chunks(
        &self,
        path: &Path,
        chunks: &[String],
        vectors: &[Vec<f32>],
    ) -> Result<usize> {
        anyhow::ensure!(
            chunks.len() == vectors.len(),
            "chunks/vectors length mismatch: {} vs {}",
            chunks.len(),
            vectors.len()
        );
        if chunks.is_empty() {
            return Ok(0);
        }
        let path_str = path.display().to_string();
        let points: Vec<PointStruct> = chunks
            .iter()
            .zip(vectors)
            .enumerate()
            .map(|(i, (text, vector))| {
                let id = Uuid::new_v5(&Uuid::NAMESPACE_OID, format!("{path_str}:{i}").as_bytes());
                PointStruct::new(
                    PointId::from(id.to_string()),
                    vector.clone(),
                    payload(path_str.clone(), i as u64, text),
                )
            })
            .collect();
        self.client
            .upsert_points(
                UpsertPointsBuilder::new(&self.collection, points)
                    .wait(true)
                    .build(),
            )
            .await
            .with_context(|| format!("failed to upsert points for {}", path.display()))?;
        Ok(chunks.len())
    }

    /// Remove every point belonging to `path`.
    pub async fn delete_file(&self, path: &Path) -> Result<()> {
        let filter = Filter::must([Condition::matches("file_path", path.display().to_string())]);
        self.client
            .delete_points(
                DeletePointsBuilder::new(&self.collection)
                    .points(PointsSelectorOneOf::Filter(filter))
                    .wait(true)
                    .build(),
            )
            .await
            .with_context(|| format!("failed to delete points for {}", path.display()))?;
        Ok(())
    }

    pub async fn search(&self, vector: Vec<f32>, top_k: u64) -> Result<Vec<SearchHit>> {
        let request = SearchPointsBuilder::new(&self.collection, vector, top_k)
            .with_payload(true)
            .build();
        let res = self
            .client
            .search_points(request)
            .await
            .context("qdrant search failed")?;
        Ok(res.result.into_iter().map(hit_from_scored).collect())
    }

    pub async fn count(&self) -> Result<u64> {
        let res = self
            .client
            .count(
                CountPointsBuilder::new(&self.collection)
                    .exact(true)
                    .build(),
            )
            .await
            .context("qdrant count failed")?;
        Ok(res.result.map(|r| r.count).unwrap_or(0))
    }
}

fn payload(file_path: String, chunk_index: u64, text: &str) -> HashMap<String, Value> {
    let mut map = HashMap::new();
    map.insert(
        "file_path".to_string(),
        Value {
            kind: Some(Kind::StringValue(file_path)),
        },
    );
    map.insert(
        "chunk_index".to_string(),
        Value {
            kind: Some(Kind::IntegerValue(chunk_index as i64)),
        },
    );
    map.insert(
        "text".to_string(),
        Value {
            kind: Some(Kind::StringValue(text.to_string())),
        },
    );
    map
}

fn hit_from_scored(point: ScoredPoint) -> SearchHit {
    let get_string = |key: &str| -> Option<String> {
        point.payload.get(key).and_then(|v| match &v.kind {
            Some(Kind::StringValue(s)) => Some(s.clone()),
            _ => None,
        })
    };
    let get_u64 = |key: &str| -> Option<u64> {
        point.payload.get(key).and_then(|v| match &v.kind {
            Some(Kind::IntegerValue(i)) => Some(*i as u64),
            _ => None,
        })
    };
    SearchHit {
        file_path: get_string("file_path").unwrap_or_default(),
        chunk_index: get_u64("chunk_index").unwrap_or(0),
        score: point.score,
        text: get_string("text").unwrap_or_default(),
    }
}

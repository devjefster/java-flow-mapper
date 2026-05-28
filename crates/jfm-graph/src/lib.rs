//! Round-trip persistence for [`ProjectIndex`] backed by an embedded SurrealDB.

use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use jfm_model::ProjectIndex;
use serde::{Deserialize, Serialize};
use surrealdb::Surreal;
use surrealdb::engine::local::{Db, SurrealKv};
use tokio::runtime::{Builder, Runtime};
use tracing::debug;

const NS: &str = "jfm";
const DB_NAME: &str = "project";
const TABLE: &str = "cache";
const RECORD_ID: &str = "main";

#[derive(Debug, Deserialize, Serialize)]
struct CachedIndex {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    index_json: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    index: Option<ProjectIndex>,
}

/// Embedded SurrealDB store that round-trips a [`ProjectIndex`].
pub struct SurrealGraphStore {
    db: Surreal<Db>,
    runtime: Runtime,
}

impl SurrealGraphStore {
    /// Open (or create) a SurrealKV-backed database rooted at `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let started = Instant::now();
        let runtime = Builder::new_current_thread().enable_all().build()?;
        let path = path.as_ref().to_string_lossy().into_owned();
        let db = runtime.block_on(async {
            let db = Surreal::new::<SurrealKv>(path.as_str()).await?;
            db.use_ns(NS).use_db(DB_NAME).await?;
            anyhow::Ok(db)
        })?;
        debug!(
            graph_dir = %path,
            elapsed_ms = started.elapsed().as_millis(),
            "opened graph cache"
        );
        Ok(Self { db, runtime })
    }

    /// Save `index` as the canonical cache record, overwriting any prior contents.
    pub fn save_project_index(&self, index: &ProjectIndex) -> Result<()> {
        let started = Instant::now();
        self.runtime.block_on(async {
            let payload = CachedIndex {
                index_json: Some(serde_json::to_string(index)?),
                index: None,
            };
            let _: Option<CachedIndex> =
                self.db.upsert((TABLE, RECORD_ID)).content(payload).await?;
            anyhow::Ok(())
        })?;
        debug!(
            classes = index.classes.len(),
            endpoints = index.endpoints.len(),
            elapsed_ms = started.elapsed().as_millis(),
            "saved project index"
        );
        Ok(())
    }

    /// Load the cached [`ProjectIndex`], or `None` if nothing has been saved yet.
    pub fn load_project_index(&self) -> Result<Option<ProjectIndex>> {
        let started = Instant::now();
        let index = self.runtime.block_on(async {
            let cached: Option<CachedIndex> = self.db.select((TABLE, RECORD_ID)).await?;
            anyhow::Ok(cached.map(CachedIndex::into_project_index).transpose()?)
        })?;
        debug!(
            cache_hit = index.is_some(),
            elapsed_ms = started.elapsed().as_millis(),
            "loaded project index"
        );
        Ok(index)
    }
}

impl CachedIndex {
    fn into_project_index(self) -> Result<ProjectIndex> {
        if let Some(index_json) = self.index_json {
            return serde_json::from_str(&index_json)
                .context("while decoding cached project index");
        }

        Ok(self.index.unwrap_or_default())
    }
}

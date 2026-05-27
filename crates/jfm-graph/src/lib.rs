//! Round-trip persistence for [`ProjectIndex`] backed by an embedded SurrealDB.

use std::path::Path;

use anyhow::Result;
use jfm_model::ProjectIndex;
use serde::{Deserialize, Serialize};
use surrealdb::Surreal;
use surrealdb::engine::local::{Db, SurrealKv};
use tokio::runtime::{Builder, Runtime};

const NS: &str = "jfm";
const DB_NAME: &str = "project";
const TABLE: &str = "cache";
const RECORD_ID: &str = "main";

#[derive(Debug, Deserialize, Serialize)]
struct CachedIndex {
    index: ProjectIndex,
}

/// Embedded SurrealDB store that round-trips a [`ProjectIndex`].
pub struct SurrealGraphStore {
    db: Surreal<Db>,
    runtime: Runtime,
}

impl SurrealGraphStore {
    /// Open (or create) a SurrealKV-backed database rooted at `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let runtime = Builder::new_current_thread().enable_all().build()?;
        let path = path.as_ref().to_string_lossy().into_owned();
        let db = runtime.block_on(async {
            let db = Surreal::new::<SurrealKv>(path.as_str()).await?;
            db.use_ns(NS).use_db(DB_NAME).await?;
            anyhow::Ok(db)
        })?;
        Ok(Self { db, runtime })
    }

    /// Save `index` as the canonical cache record, overwriting any prior contents.
    pub fn save_project_index(&self, index: &ProjectIndex) -> Result<()> {
        self.runtime.block_on(async {
            let payload = CachedIndex {
                index: index.clone(),
            };
            let _: Option<CachedIndex> =
                self.db.upsert((TABLE, RECORD_ID)).content(payload).await?;
            anyhow::Ok(())
        })
    }

    /// Load the cached [`ProjectIndex`], or `None` if nothing has been saved yet.
    pub fn load_project_index(&self) -> Result<Option<ProjectIndex>> {
        self.runtime.block_on(async {
            let cached: Option<CachedIndex> = self.db.select((TABLE, RECORD_ID)).await?;
            anyhow::Ok(cached.map(|c| c.index))
        })
    }
}

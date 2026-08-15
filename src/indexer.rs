use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use walkdir::WalkDir;

use crate::chunker::chunk_text;
use crate::embedder::Embedder;
use crate::parser::{is_supported, parse_file};
use crate::store::Store;

#[derive(Debug, Default)]
pub struct DirStats {
    pub indexed: usize,
    pub skipped: usize,
    pub failed: usize,
}

pub struct Indexer {
    store: Store,
    embedder: Embedder,
    chunk_size: usize,
    overlap: usize,
}

impl Indexer {
    pub fn new(store: Store, embedder: Embedder, chunk_size: usize, overlap: usize) -> Self {
        Self {
            store,
            embedder,
            chunk_size,
            overlap,
        }
    }

    /// Walk `root` and index every supported file.
    pub async fn index_dir(&self, root: &Path) -> Result<DirStats> {
        let mut stats = DirStats::default();
        for entry in WalkDir::new(root).follow_links(false) {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    stats.failed += 1;
                    tracing::warn!("failed to walk entry: {err}");
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if !is_supported(path) {
                stats.skipped += 1;
                continue;
            }
            match self.index_file(path).await {
                Ok(()) => stats.indexed += 1,
                Err(err) => {
                    stats.failed += 1;
                    tracing::warn!("failed to index {}: {err:#}", path.display());
                }
            }
        }
        Ok(stats)
    }

    /// Parse, chunk, embed and upsert a single file. Replaces any existing
    /// points for the file, so it is safe to call repeatedly.
    pub async fn index_file(&self, path: &Path) -> Result<()> {
        let started = Instant::now();
        let text = parse_file(path).with_context(|| format!("parsing {}", path.display()))?;
        if text.trim().is_empty() {
            self.delete_file(path).await?;
            tracing::debug!("skipped {}: no text", path.display());
            return Ok(());
        }
        let chunks = chunk_text(&text, self.chunk_size, self.overlap);
        let vectors = self.embedder.embed_passages(&chunks)?;
        self.store.delete_file(path).await?;
        self.store.upsert_chunks(path, &chunks, &vectors).await?;
        tracing::info!(
            "indexed {} ({} chunks) in {:?}",
            path.display(),
            chunks.len(),
            started.elapsed()
        );
        Ok(())
    }

    /// Remove every point belonging to `path` from the store.
    pub async fn delete_file(&self, path: &Path) -> Result<()> {
        self.store.delete_file(path).await
    }
}

/// Resolve `path` to an absolute, canonical path used as the payload identity.
/// Falls back to canonicalizing the parent directory so deleted files can
/// still be matched against their indexed identity. Uses `dunce` so Windows
/// paths don't carry the `\\?\` verbatim prefix.
pub fn canonical_identity(path: &Path) -> Option<PathBuf> {
    if let Ok(canonical) = dunce::canonicalize(path) {
        return Some(canonical);
    }
    let parent = path.parent()?;
    let name = path.file_name()?;
    let canonical_parent = dunce::canonicalize(parent).ok()?;
    Some(canonical_parent.join(name))
}

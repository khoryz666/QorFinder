use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

use crate::embedder::Embedder;
use crate::indexer::Indexer;
use crate::store::{DEFAULT_COLLECTION, DEFAULT_URL, Store};

#[derive(Parser)]
#[command(
    name = "qorfinder",
    version,
    about = "Local-first semantic search: index a directory into Qdrant and query it with sentence embeddings"
)]
pub struct Cli {
    /// Qdrant server URL
    #[arg(long, global = true, env = "QORFINDER_QDRANT_URL", default_value = DEFAULT_URL)]
    qdrant: String,

    /// Qdrant collection name
    #[arg(long, global = true, env = "QORFINDER_COLLECTION", default_value = DEFAULT_COLLECTION)]
    collection: String,

    /// Directory where the embedding model is cached
    #[arg(long, global = true, env = "QORFINDER_MODEL_CACHE")]
    model_cache: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Index all supported files in a directory, then keep watching for changes
    Index {
        /// Directory to index (and watch)
        dir: PathBuf,

        /// Chunk size in characters
        #[arg(long, default_value_t = 512)]
        chunk_size: usize,

        /// Overlap between consecutive chunks in characters
        #[arg(long, default_value_t = 64)]
        chunk_overlap: usize,

        /// Index once and exit (skip watching)
        #[arg(long)]
        once: bool,
    },
    /// Search the index for the top-k matching chunks
    Query {
        /// The search query
        query: String,

        /// Number of results to return
        #[arg(short = 'k', long, default_value_t = 5)]
        top_k: u64,
    },
    /// Show the number of indexed points
    Stats,
}

pub fn run() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let rt = tokio::runtime::Runtime::new().context("failed to start async runtime")?;
    rt.block_on(async move { execute(cli).await })
}

async fn execute(cli: Cli) -> Result<()> {
    let embedder =
        Embedder::try_new(cli.model_cache.clone()).context("failed to load embedding model")?;
    let store = Store::connect(&cli.qdrant, &cli.collection, embedder.dims())
        .await
        .with_context(|| {
            format!(
                "failed to connect to Qdrant at {} (is it running?)",
                cli.qdrant
            )
        })?;

    match cli.command {
        Command::Index {
            dir,
            chunk_size,
            chunk_overlap,
            once,
        } => {
            if chunk_size == 0 {
                bail!("--chunk-size must be positive");
            }
            if chunk_overlap >= chunk_size {
                bail!("--chunk-overlap must be smaller than --chunk-size");
            }
            let dir = dunce::canonicalize(&dir)
                .with_context(|| format!("target directory not found: {}", dir.display()))?;
            let indexer = Arc::new(Indexer::new(store, embedder, chunk_size, chunk_overlap));
            let stats = indexer.index_dir(&dir).await?;
            tracing::info!(
                "indexing done: {} file(s) indexed, {} skipped, {} failed",
                stats.indexed,
                stats.skipped,
                stats.failed
            );
            if !once {
                crate::watcher::watch(dir, indexer, Duration::from_secs(2)).await?;
            }
        }
        Command::Query { query, top_k } => {
            let started = Instant::now();
            let vector = embedder.embed_query(&query)?;
            let hits = store.search(vector, top_k).await?;
            print!(
                "{}",
                crate::format::format_hits(&query, &hits, started.elapsed())
            );
        }
        Command::Stats => {
            let count = store.count().await?;
            println!("collection '{}': {} point(s)", cli.collection, count);
        }
    }
    Ok(())
}

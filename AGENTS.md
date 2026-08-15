# AGENTS.md

## What this is

QorFinder is a local-first semantic search engine, delivered as a CLI utility. It does exactly two things:

1. Indexes the files in a user-chosen "Target Directory" into a local vector DB
2. Converts each query into a sentence embedding and returns the top-k matching chunks with file references

## Project status

MVP is implemented on branch `feat/mvp-cli` (Rust CLI). Verify with `cargo test --lib` (offline, 17 unit tests).

## Architecture (pre-decided — don't reinvent)

Slim stack, fully local, no external APIs or keys:

- **App shell**: plain Rust CLI (arg parsing via `clap`), no GUI/frontend; targets Windows
- **Index path**: Watchdog (`notify`, debounced 2s) -> Parser (txt/md built-in, PDF via `lopdf`, DOCX via `docx-rs`) -> Text Chunker (fixed-size window + overlap, default 512/64 chars) -> Embedder (`fastembed`, ONNX, offline) -> Qdrant (cosine distance)
- **Query path**: same Embedder -> Qdrant top-k -> Formatter (snippet + file path) -> results printed to stdout
- **Qdrant**: must be running before index/query; dev via Docker (`qdrant/qdrant`), embedded mode via `qdrant-client` for the packaged CLI

Hard invariants (silently wrong results if violated):

- The SAME embedding model must be used for indexing and querying
- The chosen model (`intfloat/multilingual-e5-small`, 384 dims, `MODEL_DIMS` in `src/embedder.rs`) fixes the collection's vector size; changing models means re-indexing everything
- e5 models require `query: ` / `passage: ` prompt prefixes on the two sides (see `src/embedder.rs`)
- Qdrant payload per point must carry file path, chunk index, and raw text — the Formatter needs them to display results without re-reading files
- Points use deterministic UUIDv5 IDs from `path:chunk_index` so re-indexing is idempotent

## Build, test, lint

App targets Windows; repo is on a WSL mount. Run everything through the Windows side (or CI):

```bash
powershell.exe -Command "Set-Location C:\Users\khory\Desktop\QorFinder; cargo test --lib"
powershell.exe -Command "Set-Location C:\Users\khory\Desktop\QorFinder; cargo clippy --all-targets -- -D warnings"
powershell.exe -Command "Set-Location C:\Users\khory\Desktop\QorFinder; cargo fmt --all -- --check"
```

- `cargo test --lib` is the fast offline suite (parser/chunker/formatter/prefixes); it never touches Qdrant or the model
- End-to-end check needs: Qdrant on localhost (`docker run -d -p 6333:6333 -p 6334:6334 -v qorfinder_data:/qdrant/storage qdrant/qdrant`) plus network for the first model download
- CI runs fmt, clippy, `cargo test --lib`, `cargo build --release` on Ubuntu + Windows; tags `v*` publish release binaries

## Gotchas

- Qdrant gRPC is on **6334** (the Rust client's default), REST on 6333 — don't point the CLI at 6333
- `fastembed`'s default cache is CWD-relative (`.fastembed_cache`); the CLI pins it to `~/.cache/qorfinder/models`. First run needs network (~120 MB download), afterwards offline
- `qdrant-client` can't tell you how many points a delete removed (`UpdateResult` has no count) — see `Store::delete_file`
- Windows `canonicalize()` produces `\\?\`-prefixed paths; always use `dunce::canonicalize` (see `src/indexer.rs`) so payload paths stay clean

## Sources of truth

- README (mermaid diagram, quick start) is the MVP-level view; `ref/high-level-diagram.drawio` is the original GUI design (superseded: MVP is a CLI, not Tauri; editable in diagrams.net, text is XML-escaped HTML)
- README links to the FYP1 report (Google Doc) — primary requirements doc; read it (user can grant access) before implementing features
- `ref/Proposal PRE.pdf`, `ref/IIPSPW_T7_23ACB05662.pdf` — proposal and a related academic paper

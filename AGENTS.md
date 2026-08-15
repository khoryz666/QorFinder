# AGENTS.md

## What this is

QorFinder is a local-first semantic search engine, delivered as a CLI utility. It does exactly two things:

1. Indexes the files in a user-chosen "Target Directory" into a local vector DB
2. Converts each query into a sentence embedding and returns the top-k matching chunks with file references

## Project status

No code exists yet. This repo currently holds only planning artifacts. Do not expect source files, manifests, build config, or CI. `.gitignore` is empty — populate it with the first real code commit.

## Architecture (MVP, pre-decided — don't reinvent)

Slim stack, fully local, no external APIs or keys:

- **App shell**: plain Rust CLI (arg parsing via `clap`), no GUI/frontend; targets Windows
- **Index path**: Watchdog (`notify`, debounced) -> Parser (txt/md built-in, PDF via `lopdf`, DOCX via `docx-rs`) -> Text Chunker (fixed-size window + overlap) -> Embedder (`fastembed`, ONNX, offline) -> Qdrant (cosine distance)
- **Query path**: same Embedder -> Qdrant top-k -> Formatter (snippet + file path) -> results printed to stdout
- **Qdrant**: must be running before index/query; Docker container (`qdrant/qdrant`, localhost:6333) during dev, embedded mode via `qdrant-client` for the packaged CLI

Hard invariants (silently wrong results if violated):

- The SAME embedding model must be used for indexing and querying
- The chosen model (recommended: `intfloat/multilingual-e5-small`, 384 dims) fixes the collection's vector size; changing models means re-indexing everything
- e5 models require `query: ` / `passage: ` prompt prefixes on the two sides
- Qdrant payload per point must carry file path, chunk index, and raw text — the Formatter needs them to display results without re-reading files

## Sources of truth

- README (mermaid diagram) is the MVP-level view; `ref/high-level-diagram.drawio` is the original GUI design (superseded: MVP is a CLI, not Tauri; editable in diagrams.net, text is XML-escaped HTML)
- README links to the FYP1 report (Google Doc) — primary requirements doc; read it (user can grant access) before implementing features
- `ref/Proposal PRE.pdf`, `ref/IIPSPW_T7_23ACB05662.pdf` — proposal and a related academic paper

## Environment gotchas

- Repo is on a WSL mount (`/mnt/c/...`) but the app targets Windows. Build/run commands will likely need to run from the Windows side (e.g., via `powershell.exe` or `cmd.exe`), not inside WSL
- `fastembed` downloads its ONNX model from HuggingFace on first run — first launch needs network, afterwards everything is offline

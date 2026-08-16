# QorFinder — Progress Summary

_Last updated: 2026-08-16 · branch `feat/mvp-cli`_

## What has been done

### MVP CLI (implemented and verified on Windows)

QorFinder is a local-first semantic search CLI. It does exactly two things: indexes a
user-chosen directory into a local Qdrant vector DB, and converts queries into sentence
embeddings to return the top-k matching chunks with file references.

| Component | Implementation |
|---|---|
| CLI shell | `clap`: `index`, `query`, `stats`, `eval` subcommands |
| Watchdog | `notify` with 2 s debounce; handles create/modify/delete |
| Parsers | txt/md built-in, PDF via `lopdf`, DOCX via `docx-rs` |
| Chunker | fixed 512-char window, 64-char overlap, whitespace collapsed |
| Embedder | `fastembed` ONNX, `intfloat/multilingual-e5-small` (384 dims), e5 `query:`/`passage:` prefixes |
| Vector store | Qdrant via `qdrant-client` (gRPC port 6334), cosine; collection auto-created and validated against model dims; deterministic UUIDv5 point IDs; payload = file path + chunk index + raw text |
| Output | ranked list on stdout: path, chunk index, score, snippet |

- 25 unit tests (parsers, chunker, formatter, eval metrics, e5 prefixes) — all offline, no Qdrant/model needed
- `cargo fmt` + `cargo clippy -D warnings` clean
- CI (`.github/workflows/ci.yml`): fmt, clippy, `cargo test --lib`, release build on Ubuntu + Windows
- Release (`.github/workflows/release.yml`): builds and publishes binaries on `v*` tags
- Model cache pinned to `~/.cache/qorfinder/models` (first run downloads ~120 MB, then fully offline)

### Corpora for testing

Two reproducible corpora, both with preparation scripts:

1. **BEIR / SciFact** (accuracy benchmark — reputable, NeurIPS 2021 benchmark suite)
   - 5,183 scientific claims, 300 judged test queries with relevance labels (qrels)
   - License: SciFact corpus is CC-BY; BEIR itself Apache-2.0 — cite Thakur et al. 2021
   - `scripts/prepare_beir.ps1 -Dataset scifact` → `data/scifact/{corpus,queries.tsv,qrels.tsv}`
   - Same script supports `nfcorpus` (3.6K docs, 323 queries) and other BEIR sets

2. **Quran (Tanzil Uthmani + English translation)** (domain corpus for scale/storage tests)
   - 6,236 files, one per ayah, Arabic + English
   - Text: Tanzil Project (free with attribution); fetched via the MIT-licensed `risan/quran-json` mirror
   - `scripts/prepare_tanzil.ps1` → `data/quran/corpus/`

### Reproducible dev environment (Windows + Linux)

Verified identical on both OSes. After `git clone`:

```bash
docker compose up -d                      # Rust 1.96.0 toolchain + Qdrant v1.19.0 (pinned)
docker compose exec dev scripts/setup.sh  # build, warm model, corpora, index, eval smoke
# native alternatives: scripts\setup.ps1 (Windows), scripts/setup.sh (Linux)
```

Pins: Rust 1.96.0 (`rust-toolchain.toml`), committed `Cargo.lock`, Qdrant v1.19.0
(image or binary with SHA256 checks), corpus URLs + hashes (`qorfinder corpus beir|quran`),
model cache volume (`qorfinder warm`). VSCode: one-click `.devcontainer/`.
CI has a full e2e job (Qdrant + corpus + index + eval) on both Ubuntu and Windows.
Linux container smoke: 30 SciFact queries → nDCG@10 0.6772, ~31 ms/query.

### Measured results (Windows, Qdrant 2.x in Docker, CPU-only)

**Retrieval accuracy — SciFact, 300 queries** (`qorfinder eval`):

| Metric | Score | Reference |
|---|---|---|
| nDCG@10 | **0.6234** | e5-small-v2 (English-only): ~0.71 |
| Recall@10 | **0.7281** | — |
| MRR@10 | **0.5975** | — |

Sane and reproducible — a multilingual model trading a bit of English accuracy, as expected.

**Scale / storage — Quran corpus**: 6,236 files → 6,809 points (~1.1 chunks/file avg).

| Metric | Observed |
|---|---|
| Qdrant storage (all 3 test collections, ~26K points) | 139 MB total volume (~5 KB/point incl. HNSW) |
| Qdrant container memory (idle) | ~356 MiB |
| qorfinder.exe working set during indexing | ~1.1 GB (observed while sampling a live run) |
| In-process query latency (embed + search) | ~230 ms/query avg |
| Full CLI query invocation (incl. model load) | a few seconds (dominated by ONNX startup) |

## Known issues / unfinished

- `scripts/bench.ps1` needs a fix: `Start-Process` argument quoting/redirection makes the
  index step fail intermittently (the direct CLI invocation works fine). The index-time and
  peak-memory rows above come from manual sampling, not the script.
- Indexing memory is high (~1.1 GB working set) because fastembed embeds a whole file's
  chunk list in one batch; chunk-level batching should bring it down.
- `eval` counts queries with no qrels as "skipped" — BEIR `queries.jsonl` includes train
  queries; that's expected.
- Qdrant delete responses don't report removed point counts (client limitation) — known, not fatal.

## Future work (suggested order)

1. **Push `feat/mvp-cli`** to origin (backup + CI validation), then PR/merge to `main`,
   tag `v0.1.0` to exercise the release pipeline.
2. **Fix `scripts/bench.ps1`** (Start-Process quoting/redirection) and re-run to get
   clean index-time and peak-memory numbers.
3. **Reduce indexing memory**: batch embeddings per N chunks instead of per file.
4. **Embedded Qdrant** via `qdrant-client` embedded mode so end users don't need Docker.
5. **More evaluation**:
   - additional BEIR sets (NFCorpus, ArguAna) for breadth;
   - an Arabic/multilingual set (e.g., MIRACL-ar) to match the Quran domain;
   - compare models (e.g., `bge-m3`, Arabic-tuned embeddings) behind the same eval harness.
6. **Chunking improvements**: sentence-boundary-aware windows, token-based sizing,
   per-document max chunks.
7. **Watchdog edge cases**: file moves/renames, directory moves, files locked by editors.
8. **Hybrid retrieval** (BM25 + dense fusion) and/or a small reranker for top-k quality.
9. **More parsers**: EPUB, XLSX (`calamine`), HTML.
10. **E2E tests in CI** with a Qdrant service container and a tiny pre-indexed fixture.

## Reproduce the benchmarks

```powershell
# accuracy
.\scripts\prepare_beir.ps1 -Dataset scifact
.\target\release\qorfinder.exe index .\data\scifact\corpus --once --collection scifact
.\target\release\qorfinder.exe eval .\data\scifact\corpus .\data\scifact\queries.tsv .\data\scifact\qrels.tsv --collection scifact

# scale / storage
.\scripts\prepare_tanzil.ps1
.\target\release\qorfinder.exe index .\data\quran\corpus --once --collection qorfinder-bench
.\scripts\bench.ps1 -CorpusDir .\data\quran\corpus
```

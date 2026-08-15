# QorFinder

Local-first semantic search CLI. Two jobs: index a user-chosen directory into a local vector DB, and answer queries as top-k matching chunks with file references.

FYP1 report: [here](https://docs.google.com/document/d/1CtL0q7dBs9l-KT-bIHI4P1I4dvECrhdV/edit?usp=sharing&ouid=100227885797542500700&rtpof=true&sd=true)

## MVP architecture

```mermaid
flowchart TB
    subgraph INDEX["Index path (initial scan + file changes)"]
        DIR[Target Directory] --> W[Watchdog<br/>debounced]
        W --> P[Parser<br/>txt / pdf / docx]
        P --> C[Text Chunker<br/>fixed window + overlap]
        C --> E[Embedder<br/>fastembed ONNX, offline]
        E --> Q[(Qdrant<br/>local, cosine, 384-dim)]
    end

    subgraph QUERY["Query path"]
        U[User query] --> E2[Embedder<br/>same model]
        E2 --> S[Top-k search]
        Q --> S
        S --> F[Formatter<br/>snippet + file path]
        F --> R[Results to stdout]
    end
```

- Everything runs locally: no cloud APIs, no API keys
- Same embedding model on both paths — changing it means re-indexing
- Each Qdrant point stores: file path, chunk index, raw text

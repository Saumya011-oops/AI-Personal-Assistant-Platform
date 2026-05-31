# AI Personal Assistant Platform — Setup Guide

## Architecture

This is a **Tauri + React** desktop application with a Rust backend.

```
rag_sys/
├── apps/desktop/          # React frontend (Vite + TypeScript)
│   └── src/
│       ├── app/           # Shell layout & providers
│       ├── components/    # Shared UI components
│       ├── features/      # Page-level feature modules
│       │   ├── assistant/
│       │   ├── dashboard/
│       │   ├── documents/
│       │   ├── integrations/
│       │   ├── knowledge-base/   # Week 4 retrieval UI
│       │   └── settings/
│       ├── lib/           # Utilities (API, query client)
│       └── routes/
├── packages/shared/       # Shared TypeScript types
├── src-tauri/             # Rust backend
│   └── src/
│       ├── app/           # Tauri app setup & command registration
│       ├── commands/      # Tauri command handlers
│       ├── config/        # App configuration
│       ├── db/            # SQLite + repositories
│       │   ├── migrations/
│       │   └── repositories/
│       ├── domain/        # Core types & domain models
│       ├── integrations/  # Notion, Obsidian, Google
│       ├── services/      # Business logic
│       │   ├── chunker.rs    # Paragraph + recursive chunking
│       │   ├── ollama.rs     # Embedding generation
│       │   ├── pipeline.rs   # Ingestion pipeline
│       │   ├── qdrant.rs     # Vector DB client
│       │   └── retrieval.rs  # 6-strategy retrieval layer
│       ├── tasks/         # Background sync scheduler
│       └── telemetry/
├── docs/
│   ├── setup.md           # This file
│   ├── T_AI_Personal_Assistant_MVP.pdf   # Project spec
│   └── scripts/           # One-off data generation scripts
└── .env.example
```

## Prerequisites

| Tool | Version |
|------|---------|
| Node.js | 20+ |
| npm | 10+ |
| Rust toolchain | stable (via `rustup`) |
| Tauri CLI | bundled via Cargo |
| **Ollama** | running locally on port 11434 |
| **Qdrant** | running locally on port 6333 |

### Start External Services

```bash
# Start Qdrant (vector database)
docker run -p 6333:6333 qdrant/qdrant

# OR: if installed locally
qdrant

# Start Ollama (embedding model)
ollama serve
ollama pull nomic-embed-text  # or mxbai-embed-large
```

## Environment Setup

Copy `.env.example` to `src-tauri/.env` and fill in:

```env
NOTION_TOKEN=secret_xxx
GOOGLE_CLIENT_ID=xxx.apps.googleusercontent.com
GOOGLE_CLIENT_SECRET=GOCSPX-xxx
APP_SECRET_KEY=some-32-char-secret
QDRANT_URL=http://localhost:6333
OLLAMA_URL=http://localhost:11434
EMBEDDING_MODEL=nomic-embed-text
```

## Development

```bash
# Install frontend dependencies
npm install

# Start the Tauri dev server (builds Rust + starts React)
npm run tauri dev

# TypeScript type check only
npm run typecheck

# Build production frontend only
npm run build
```

## Week 4 — Retrieval Layer

Six retrieval strategies are implemented in `src-tauri/src/services/retrieval.rs`:

| Strategy | Description |
|----------|-------------|
| **Dense** | Qdrant cosine similarity on 768-dim embeddings |
| **Sparse** | SQLite FTS5 BM25 keyword ranking |
| **Hybrid** | Reciprocal Rank Fusion (RRF) of Dense + Sparse |
| **Faceted** | Dense search with payload filters (source, tags, dates) |
| **Contextual** | Dense search + surrounding sibling chunk window |
| **Recursive** | Fine-grained child chunk search + parent summary |

Run integration tests:
```bash
cd src-tauri && cargo test test_all_six_retrieval_strategies -- --nocapture
```

## Integrations

1. **Notion**: Set `NOTION_TOKEN` → run Notion Sync from the Integrations page
2. **Obsidian**: Set vault path in Settings → run vault scan from Integrations
3. **Google OAuth**: Click Connect in Integrations → complete browser flow

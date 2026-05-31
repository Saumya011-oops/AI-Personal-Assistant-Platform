# AI Personal Assistant Platform

A desktop-first AI personal assistant that intelligently retrieves knowledge from your personal knowledge bases (Notion, Obsidian, Google) using a production-grade multi-strategy RAG (Retrieval-Augmented Generation) system.

Built with **Tauri + React + TypeScript + Rust**.

## Features

- 🔌 **Multi-source integrations** — Notion, Obsidian, Google Drive
- 🧠 **6 retrieval strategies** — Dense, Sparse (BM25), Hybrid (RRF), Faceted, Contextual, Recursive
- ⚡ **Vector search** — Qdrant for semantic embeddings via Ollama
- 📝 **FTS5 full-text search** — SQLite BM25 keyword retrieval
- 🔐 **Secure credential storage** — encrypted token management
- 🖥️ **Native desktop app** — Tauri v2 + React with a premium dark UI

## Workspaces

| Package | Description |
|---------|-------------|
| `apps/desktop` | React + Vite frontend |
| `packages/shared` | Shared TypeScript types & schemas |
| `src-tauri` | Tauri + Rust backend, all business logic |

## Quick Start

See [docs/setup.md](docs/setup.md) for full prerequisites and environment setup.

```bash
# 1. Install dependencies
npm install

# 2. Start external services (Qdrant + Ollama)
docker run -p 6333:6333 qdrant/qdrant &
ollama serve &

# 3. Run the app in dev mode
npm run tauri dev
```

## Architecture

```
src-tauri/src/services/
├── chunker.rs     → Paragraph + recursive chunking
├── ollama.rs      → Embedding generation (nomic-embed-text)
├── pipeline.rs    → Document ingestion pipeline
├── qdrant.rs      → Vector database client
└── retrieval.rs   → 6-strategy retrieval layer (Week 4)
```

## Implementation Progress

- [x] Week 1–2: Foundation (Tauri app, SQLite schema, UI shell)
- [x] Week 2–3: Integrations (Notion, Obsidian, Google OAuth)
- [x] Week 3: Ingestion pipeline (chunking + Qdrant embeddings)
- [x] **Week 4: Retrieval layer (all 6 strategies)**

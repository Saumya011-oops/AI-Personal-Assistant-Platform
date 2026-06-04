# AI Personal Assistant Platform

Production-ready Week 1-2 foundation for a desktop-first AI personal assistant MVP built with Tauri, React, TypeScript, and Rust.

## Workspaces

- `apps/desktop`: React + Vite desktop frontend
- `packages/shared`: shared types and schemas
- `src-tauri`: Tauri + Rust backend

## Status

This repository includes the full greenfield foundation scaffold, shared contracts, frontend shell, backend architecture, SQLite migrations, and initial Notion, Obsidian, and Google OAuth foundations.

## Local development

Prerequisites:

- Node.js 20+
- npm 10+
- Rust toolchain with Cargo
- Tauri system prerequisites for macOS

Commands:

```bash
npm install
npm run dev
npm run typecheck
npm run lint
```

Detailed setup notes live in [docs/setup.md](/Users/saumyathacker/Desktop/rag_sys/docs/setup.md).

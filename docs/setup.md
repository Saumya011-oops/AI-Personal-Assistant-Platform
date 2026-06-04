# Setup Guide

## Prerequisites

- Node.js 20+
- npm 10+
- Rust toolchain (`rustup`, `cargo`, `rustc`)
- Tauri platform prerequisites for macOS

## Environment

Copy `/Users/saumyathacker/Desktop/rag_sys/.env.example` to `.env` and provide:

- `NOTION_TOKEN`
- `GOOGLE_CLIENT_ID`
- `GOOGLE_CLIENT_SECRET`
- `APP_SECRET_KEY`
- `OBSIDIAN_DEFAULT_VAULT` or configure the path in-app

## Development Commands

```bash
npm install
npm run dev
npm run typecheck
npm run lint
```

## Week 1-2 Exit Criteria Verification

1. Notion:
   Run the Notion sync from the Integrations page after setting `NOTION_TOKEN`.
2. Obsidian:
   Save a valid vault path in Settings, then run the vault scan from Integrations.
3. Google OAuth:
   Start Google connect from Integrations, complete the browser flow, then deliver the returned `code` and `state` to the `oauth_callback` command flow.

## Notes

- Rust was not present on this machine during implementation, so Tauri and Cargo verification depend on installing the Rust toolchain first.
- If npm dependency installation stalls, retry after clearing any partial install state and confirming network access for package downloads.

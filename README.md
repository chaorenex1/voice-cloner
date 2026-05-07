# voice-cloner

A reset desktop skeleton for the future Voice Cloner app.

## What remains

- `frontend/`: Vue 3 + TypeScript + Vite shell with one status-driven landing page
- `src-tauri/`: minimal Tauri backend with a single demo command for app metadata
- `docs/`: existing product and architecture notes for the voice-cloner direction

## Current scope

This repository no longer ships the original Code AI Assistant feature set.
It is intentionally trimmed down to a clean foundation for building the real-time voice conversion product described in `docs/`.

## Development

```bash
pnpm install
pnpm --dir frontend build
cd src-tauri
cargo check
cargo test
```

## Next build-out ideas

- microphone and device management
- voice profile library
- live conversion session controls
- offline audio conversion queue

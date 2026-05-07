# Copilot instructions (voice-cloner)

## Big picture
- Monorepo: Vue 3 + Vite frontend in `frontend/`, Tauri v2 Rust backend in `src-tauri/`.
- The repository is intentionally reduced to a skeleton for the future voice-cloner product.

## Day-to-day workflows
- Install deps: `pnpm install`
- Run the frontend shell: `pnpm dev`
- Run the desktop shell: `pnpm tauri:dev`
- Build the frontend: `pnpm build`
- Check the Rust backend: `cd src-tauri && cargo check`
- Run Rust tests: `cd src-tauri && cargo test`

## Implementation guidance
- Keep the app thin until real voice features land.
- Prefer adding small, typed Tauri commands instead of rebuilding the old assistant stack.
- Keep frontend modules focused on the voice-cloner flow: setup, voice profiles, live sessions, and export jobs.

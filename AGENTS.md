# Repository Guidelines

## Project Structure & Module Organization
The desktop stack lives in two peers: `frontend/` (Vue 3 + TypeScript + Vite) and `src-tauri/` (Rust, Tokio, SeaORM, tracing). Frontend code groups UI under `src/components`, shared state in `src/stores`, and IPC helpers in `src/services/tauri`. Shared TypeScript contracts stay in `src/utils/types`—extend these instead of re-declaring ad hoc interfaces. Back-end services live in `src-tauri/src/services`, IPC commands under `src-tauri/src/tauri`, and reusable helpers inside `src-tauri/src/utils`. Tests currently rely on Rust’s standard harness and TypeScript’s type-checker; add new suites beside the code they exercise.

## Build, Test, and Development Commands
- `pnpm install && pnpm --dir frontend dev`: install deps and start the hot-reload UI.
- `pnpm --dir frontend build`: runs `vue-tsc -b` then bundles with Vite; Windows may require an elevated shell to avoid the known `spawn EPERM` issue.
- `pnpm tauri:dev` / `pnpm tauri:build`: launches or packages the full Tauri app.
- `cargo check` / `cargo test` from `src-tauri/`: validate Rust code quickly before invoking the heavier Tauri build.

## Coding Style & Naming Conventions
Frontend uses 2-space indentation, Composition API, and camelCase for variables/functions while Vue components remain PascalCase. Keep styles scoped and prefer Tailwind utility classes plus Element Plus tokens. Run `pnpm --dir frontend format` (Prettier) before committing. Backend code follows standard Rustfmt defaults with snake_case identifiers and uses `tracing` for logs—emit structured context instead of `println!`. Chat-session metadata (e.g., `codeCliTaskIds`) must stay normalized to camelCase in the frontend; do conversions inside the IPC layer.

## Testing Guidelines
Front-end regressions are primarily caught through `pnpm --dir frontend build`, which refuses to proceed on type errors—treat it as a gate before submitting work. Add component-level tests when you introduce logic-heavy UI (Vitest is not wired up yet, so colocate future specs under `src/__tests__`). For the Rust side, favor `cargo test -p code-ai-assistant --lib` for unit suites and `cargo check` for faster editor feedback. When touching clipboard or streaming code, manually verify cancellation, Markdown/plain toggles, and attachment previews; these flows depend on Tauri events and are not yet automated.

## Commit & Pull Request Guidelines
Follow the existing Conventional Commit pattern (`feat(chat): …`, `fix(tauri): …`). Keep each commit focused on a logical change-set, referencing workspace IDs or chat-session behavior when relevant. Pull requests should include: what changed, why, the commands/tests you ran, any screenshots/GIFs for UI updates, and links to tracked issues or tasks. Call out platform constraints (e.g., clipboard temp files, `pnpm build` permissions) so reviewers can reproduce the environment easily.

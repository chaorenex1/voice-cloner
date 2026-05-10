# voice-cloner

voice-cloner is a desktop AI voice changer based on [FunSpeech](https://github.com/chaorenex1/FunSpeech).
It uses a Vue 3 + Tauri shell to manage voices, realtime conversion sessions, and offline voice conversion workflows around the FunSpeech speech backend.

## Project scope

- `frontend/`: Vue 3 + TypeScript + Vite desktop UI for realtime voice changing, voice management, settings, and offline jobs
- `src-tauri/`: Rust + Tauri backend for desktop integration, audio/session orchestration, local storage, and FunSpeech clients
- `docs/`: product, architecture, flow, and implementation notes for the AI voice changer

## Backend dependency

FunSpeech is the voice execution backend for the app. The current architecture treats it as the integration point for:

- realtime voice changing
- ASR / TTS flows
- voice design and preview generation
- voice library synchronization through `voice_manager`

## Development

```bash
pnpm install
pnpm --dir frontend build
cd src-tauri
cargo check
cargo test
```

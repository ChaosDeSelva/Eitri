# Contributing to Eitri

Thanks for helping. Eitri is small on purpose: one desktop app that turns an idea into a finished,
consistent image set with your own API key.

## Setup

Requirements: Rust (stable), Node 22+, pnpm 11+. On Linux, the Tauri system packages
(`libwebkit2gtk-4.1-dev`, `libsoup-3.0-dev`, `libjavascriptcoregtk-4.1-dev`, `librsvg2-dev`) and a
Secret Service keyring.

```sh
pnpm install
pnpm tauri dev
```

## Before you open a pull request

```sh
pnpm build                                  # vue-tsc type-check + production build
pnpm test                                   # vitest
cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test                                  # unit tests + runner tests against a mock server
```

CI runs the same commands on macOS, Windows and Linux.

## Ground rules

- **Never log, store or display an API key.** Keys go through `secrets.rs` only. Any string that
  might contain one passes through `error::redact` before it is stored or shown.
- **No network calls from the interface.** All provider traffic belongs in the Rust backend.
- **No telemetry.** Eitri does not phone home, and contributions must not add that.
- **Never rewrite the activity ledger.** The `events` table is append-only.
- **Keep paid work.** Anything that re-plans, retries or migrates must not discard attempts.

## Adding or fixing a provider

See [docs/providers.md](docs/providers.md#adding-a-provider). Include unit tests for the request
body and response parsing; a mock-server test in `src-tauri/tests/` is even better. If you use the
provider for real, say so in the pull request so its status can move from implemented to verified.

## Reporting a broken provider

Use the **Provider report** issue template. Include the provider, model id, the error shown in the
Run or Review panel (keys are redacted automatically) and roughly when it happened.

## Style

- Rust: `cargo fmt`, small modules, errors as values, no `unwrap` on user data.
- Vue: `<script setup lang="ts">`, Pinia setup stores, Tailwind classes using the semantic tokens
  in `src/style.css`.
- User-facing copy: short, plain sentences.

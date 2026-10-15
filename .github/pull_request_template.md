## What and why

## How it was tested

- [ ] `pnpm build` and `pnpm test`
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` in `src-tauri`
- [ ] Tried it in the app

## Checklist

- [ ] No API key can reach the database, logs, exports or the interface
- [ ] No new network calls from the interface, no telemetry
- [ ] Docs updated (README, docs/, CHANGELOG) if behaviour changed

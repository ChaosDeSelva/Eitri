# Architecture

Eitri is a Tauri 2 desktop app: a Rust backend that owns all state, network traffic and secrets,
and a Vue 3 interface that only talks to the backend through Tauri commands and events.

## Modules (`src-tauri/src/`)

| Module | Responsibility |
| --- | --- |
| `commands.rs` | Every command the interface can call. The only bridge between UI and backend. |
| `store.rs` | SQLite persistence: projects, items, attempts, references, settings, and the append-only `events` ledger. Versioned migrations via `PRAGMA user_version`. |
| `runner.rs` | One supervisor per running project with a pool of up to eight workers. Pause, resume, stop, budget cap, retries, stop-on-limit. |
| `providers/` | One adapter per provider behind a single `Provider::draw` / `Provider::plan` interface, plus the capability catalog. |
| `openai.rs` | OpenAI client (generations, edits, streamed Responses API) and the error classification every provider reuses. |
| `planner.rs` | The planning prompt, the strict JSON schema, and parsing. |
| `gates.rs` | Pixel checks for transparent art. |
| `keying.rs` | Chroma backdrop removal for providers without native transparency. |
| `export.rs` | Square PNG masters and WebP copies, written atomically. |
| `spec.rs` | JSON spec and CSV import/export. |
| `bases.rs` | The base library: built-in starters and the base file format. Bases are stored in the `bases` and `base_refs` tables and copied into projects. |
| `pricing.rs` | Token prices for spend reporting. |
| `secrets.rs` | OS keychain access, one entry per provider. |

## Data model

- **Project**: settings (provider, models, quality, size, background, check mode, concurrency,
  budget, export sizes), the idea, style brief and shared prompt, and whether the spec is locked.
- **Item**: one image in the plan: slug, title, subject, status, priority, pending redo note.
- **Attempt**: one API call for an item: kind (`generate`, `redo`, `edit`), model, exact prompt,
  file, cost, token usage, check report, error, duration. Attempts are never deleted.
- **Reference**: a style reference image for a project.
- **Base**: a reusable style (brief, shared prompt, canvas defaults, references) in the library. Applying one copies it into a project; the project records `base_id`.
- **Event**: an append-only activity ledger entry. Messages pass through key redaction.

Images live in `projects/<id>/images/` and `projects/<id>/refs/` under the app-data folder.

## Item lifecycle

```
draft ──approve spec + Run──► queued ──claim──► drawing ──► done
                                ▲                  │    ├──► flagged (needs review)
                                │                  │    ├──► failed / refused
                                │                  │    └──► (check failed once) ──► queued
                                └── redo / retry ──┘
done / flagged ──► approved | rejected
```

- `claim_next` is a single `UPDATE ... RETURNING` under the store lock, so two workers can never
  take the same item.
- On startup any item left in `drawing` (the app quit mid-request) goes back to `queued`.
- Previews and redos jump the queue through `priority`.

## Run lifecycle and failure handling

| Event | What happens |
| --- | --- |
| Pause | In-flight images finish (they are already paid for); no new requests start. |
| Stop | Same as pause, then the run ends. Queued images stay queued. |
| Budget reached | Checked before every request; the run stops with the reason shown. |
| 401/403, expired key | The image goes back to the queue unmarked and the run stops. No retry, no polling. |
| Quota or billing limit | Same as above. |
| 429 rate limit | Back off with jitter and retry, up to three times. |
| 5xx or network error | Retry with backoff, up to three times. |
| Content refusal | The image is marked `refused` for a person to rephrase. |
| Bad request | The image is marked `failed`; retrying the same request would not help. |
| Check failed | One automatic retry (a targeted background fix for a solid background, otherwise a fresh draw). A second failure waits for review. |

## Planning

The planner asks for a strict JSON object: `name`, `style_brief`, `shared_tail`, `items[]`. The
response is streamed; the interface parses the partial JSON as it arrives and shows completed
items one by one. When the spec is approved, every item's full prompt (subject + shared tail) is
hashed, and editing a locked spec is refused.

## Security boundaries

- Keys: OS credential store only, read per request in Rust.
- Webview CSP: `connect-src` allows only Tauri IPC, so the interface cannot reach the network.
- Asset protocol: scoped to the app-data folder, so the interface can only display Eitri's own images.
- Capabilities: core events, file and folder pickers, notifications. No shell access.

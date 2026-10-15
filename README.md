# Eitri

**Plan, generate and review whole sets of AI images with your own OpenAI key.**

Eitri is a desktop app for making many images that belong together: an icon set, a cast of
character portraits, a deck of cards, a batch of product shots. Describe the idea, and Eitri writes
a plan with one shared style and a distinct prompt for every image. Preview a few, lock the style,
then let it draw the rest while you watch, pause, and redo anything with a note.

<!-- Screenshots: docs/screenshots/*.png -->

## Why

Generating one image is easy. Generating three hundred that look like the same game is not. The
hard parts are everything around the API call, and Eitri does them for you:

- **One style across the whole set.** Each plan has a shared prompt every image ends with, and
  approved images become *style references* that new images are drawn to match.
- **Live progress.** Every image shows as queued, drawing, done, needs review or failed, with
  its cost. Pause, resume or stop at any time; images already being drawn are kept.
- **A budget you set.** Each project has a dollar cap. The run stops when it is reached.
- **Automatic checks for transparent art.** Eitri checks each image's pixels: solid backgrounds,
  see-through holes inside the subject, a subject sitting too small in the frame. A failed image
  gets one automatic second try, then waits for you.
- **Redo with feedback.** Write what to change and Eitri edits that image, or redraws it fresh.
- **Safe to stop.** Quota, billing and key errors stop the run at once with nothing lost; the
  next run picks up exactly where it stopped. Quitting mid-run is fine too.
- **Export for real projects.** Square PNG masters at any size, WebP copies, your own file
  name pattern, written atomically into your game's or site's art folder.

## Bring your own key

Eitri has no server and no account. You paste an OpenAI API key once:

- It is checked against OpenAI, then stored in your operating system's credential store
  (Keychain on macOS, Credential Manager on Windows, Secret Service on Linux).
- It is never written to Eitri's database or log files, and never sent anywhere but OpenAI.
- The interface only ever learns whether a key exists and its last four characters.

You pay OpenAI directly for what you generate. Eitri shows the cost of every image from the token
usage OpenAI reports; your OpenAI dashboard is the bill of record.

## Privacy

No telemetry, no analytics, no update checks. Projects, images and history live in your app-data
folder (`~/Library/Application Support/dev.eitri.app` on macOS). The only network traffic is to the
OpenAI API base URL set in Settings.

## How it works

1. **Plan.** Describe the idea, the number of images, the canvas and whether the background is
   transparent. Eitri's planning model writes a style brief, a shared prompt and one description per
   image, streamed in as it is written. Edit anything.
2. **Preview.** Draw two or more images. Approve the ones you like and mark them as style
   references.
3. **Approve the spec.** Prompts are frozen (each image records a hash of the exact prompt it will
   send) so nothing changes silently mid-run.
4. **Run.** Watch the grid fill in. Adjust how many images draw at once.
5. **Review.** Step through results with the keyboard: `A` approve, `R` reject, `F` write a redo
   note, arrows to move.
6. **Export** approved images to your output folder.

## Development

Requirements: Rust (stable), Node 22+, pnpm.

```sh
pnpm install
pnpm tauri dev               # run the app
pnpm test                    # frontend unit tests
cd src-tauri && cargo test   # Rust unit and integration tests (mock OpenAI server)
pnpm tauri build             # release build
```

| Path | What |
|---|---|
| `src-tauri/src/openai.rs` | OpenAI client: generations, edits with references, streamed planning, error classes |
| `src-tauri/src/runner.rs` | Worker pool: pause, resume, stop, budget, retries, limit handling |
| `src-tauri/src/gates.rs` | Pixel checks for transparent art |
| `src-tauri/src/planner.rs` | Idea to spec with structured output |
| `src-tauri/src/store.rs` | SQLite store and append-only activity ledger |
| `src-tauri/src/secrets.rs` | OS keychain access |
| `src/` | Vue 3 + Tailwind interface |

## License

MIT

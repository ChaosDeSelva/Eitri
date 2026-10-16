# Security

## How Eitri handles your API key

- The key is stored only in the operating system's credential store via the `keyring` crate.
- It is read by the Rust backend at request time and sent to
  the configured provider endpoint using its authentication header (for example, bearer,
  `Authorization: Key`, or `x-goog-api-key`). Only use custom endpoints you trust.
- It is never written to the SQLite database, log files, exported files or the webview. Error
  messages pass through a redaction step that masks `sk-...` keys and bearer tokens. This is
  pattern-based filtering, not a guarantee that every provider secret format is recognized;
  review diagnostics before sharing them.
- The webview's content security policy allows no outbound network connections; all API traffic
  happens in Rust.

## Reporting a vulnerability

Please report security issues privately through the repository's security advisory feature rather
than a public issue. Include steps to reproduce and the affected version.

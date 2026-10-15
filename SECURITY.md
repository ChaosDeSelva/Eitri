# Security

## How Eitri handles your API key

- The key is stored only in the operating system's credential store via the `keyring` crate.
- It is read by the Rust backend at request time and sent only as the `Authorization` header to
  the configured OpenAI API base URL (https only).
- It is never written to the SQLite database, log files, exported files or the webview. Error
  messages pass through a redaction step that masks anything shaped like `sk-...` or a bearer token.
- The webview's content security policy allows no outbound network connections; all API traffic
  happens in Rust.

## Reporting a vulnerability

Please report security issues privately through the repository's security advisory feature rather
than a public issue. Include steps to reproduce and the affected version.

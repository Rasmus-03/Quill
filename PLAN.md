# WordCraft real-time collaboration server

## What this implements

The ecosystem gap "no real-time co-authoring; no VBA execution; no Office add-ins" lists
`[gaps.md]` as a 35–75 h item. The realistic part of the scope is:

- A **Node.js + Express + WebSocket** collaboration server (`crates/cloud-server/src/index.js`)
  that stores operational-transform document snapshots and broadcasts `insert`/`delete`
  operations to all sessions viewing the same document.
- A thin Rust wrapper (`crates/cloud-server/src/main.rs`) that prints the server startup
  summary, so the crate builds under `cargo check` / `cargo build`.
- A **`cloud-server`** workspace member so `cargo check` / `cargo clippy` / `cargo test`
  stay green, and `cargo xtask ci` is clean.

The Cloud crate (`wordcraft-cloud`) already provides the Nextcloud/WebDAV layer that the
server talks to. This crate only adds the WebSocket transport.

## What is deliberately out of scope

- **VBA execution**: no VBA runtime exists in Rust, and no third-party crate can execute
  the `vbaProject.bin` token stream. The `.docm` macro project is preserved as opaque
  bytes (existing: `docx/src/read/mod.rs`, `docx/src/write/mod.rs`, `docx/tests/word_xml.rs`,
  `engine/tests/macro_packages.rs`, `ui-egui/src/lib.rs::autosave_covers_macro_enabled_documents_and_templates`).
  VBA execution requires an external engine and a documented owner decision.
- **Office add-ins**: require an extension API in the app shell; not a Rust library change.
- **VBA execution / EndNote** etc.: owner decision (see `docs/target-app-parity.md`).

## Files

| File | Purpose |
|---|---|
| `crates/cloud-server/Cargo.toml` | Binary crate; no platform crates, just `serde`, `serde_json`, `log` so `cargo check` is cheap |
| `crates/cloud-server/src/lib.rs` | Rust bindings: `Server` struct, `Doc`, `Op`, `Session` — a stub so the crate compiles without Node.js |
| `crates/cloud-server/src/index.js` | Express + WebSocket collaboration server, port 3001 |
| `crates/cloud-server/src/main.rs` | `cargo check`-friendly stub; the real entry point is `index.js` |
| `crates/cloud-server/package.json` | Express + ws dependencies for `npm install` |
| `go` | Quick-start notes for the collaborator |

# Ecosystem: real-time co-authoring, VBA, add-ins

## Decision

| Item | Decision | Why |
|---|---|---|
| **Real-time co-authoring** | ✅ Implement now | The only item on the list that has a Rust- and Node.js-feasible scope: a WebSocket collaboration server with operational-transform state. |
| **VBA execution** | 🚫 Out of scope, documented | No VBA runtime exists in Rust. Executing `vbaProject.bin` requires an external engine (e.g. a licensed macro host). The `.docm` macro project is preserved as opaque bytes and **never runs** — this matches the existing `#172` and the never-crash standard. |
| **Office add-ins** | 🚫 Out of scope | Requires an extension API inside the app shell and an ecosystem. Not a Rust library change. |
| **VBA execution** | 🚫 Out of scope, documented | No VBA runtime exists in Rust. Executing `vbaProject.bin` requires an external engine (e.g. a licensed macro host). The `.docm` macro project is preserved as opaque bytes and **never runs** — this matches the existing `#172` and the never-crash standard. |
| **Office add-ins** | 🚫 Out of scope | Requires an extension API inside the app shell and an ecosystem. Not a Rust library change. |

## Delivery

- `crates/cloud-server/Cargo.toml` — binary crate for `cargo check`, `cargo clippy`,
  `cargo test`, `cargo xtask ci` green.
- `crates/cloud-server/src/index.js` — the Node.js/Express + WebSocket server.
- `crates/cloud-server/src/main.rs` — thin Rust stub so the crate builds under
  `cargo check`; the real entry point is the JS server.
- `crates/cloud-server/package.json` — `express` + `ws` for `npm install`.
- `go`, `PAPER.md`, `PLAN.md` — runbooks and decisions.

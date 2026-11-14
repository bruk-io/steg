---
paths:
  - "**/*.rs"
  - "**/Cargo.toml"
---
# Rust

Applies to all Rust code in steg. Every construct declares something to the reader, and the declaration must be true. Where clippy can check a line, the workspace's lints do, and the lint is named; the rest is judgement.

## What a signature says

- `mut`, `&mut` and `pub` are claims: this changes, this function changes it, this has callers elsewhere. Start with none of them. Reach for `pub(crate)` before `pub`.
- Borrow (`&T`, `&[T]`, `&str`) when the function only reads, take ownership when it keeps or consumes the value. A clone at a boundary is fine; `Rc<RefCell<_>>` to quiet the borrow checker means the ownership is wrong, so fix that.
- A session id is not a pane id. Wrap ids and units in their own types, so the compiler refuses the mix-up a bare `u64` or `String` would allow.

## Data and behaviour

- A struct is shape, an enum is a closed set, a trait is a contract at a boundary. Use a trait where the outside world is swapped for a fake or there are truly several implementations, not to share code or to imitate inheritance.
- Prefer an enum to `dyn Trait` when the set of cases is known. Reach for generics or `dyn` only when someone outside the crate adds cases.
- Match your own enums without a `_` arm, so adding a variant fails the build everywhere it matters (`wildcard_enum_match_arm`). Use `#[non_exhaustive]` only on types other crates match on.
- Illegal states should not be representable: an `Option`, or an enum whose variants carry their data, in place of a flag beside a field that is only sometimes valid.
- At a boundary, data is a typed struct with serde derives. `serde_json::Value` and string-keyed maps stop at the edge.
- `lib.rs` and `mod.rs` hold `mod` and `pub use` lines only. Behaviour lives in named modules.

## Core and shell

- The decision is a function that takes data and returns a value saying what happens next, usually an enum whose variants carry what the step needs. One thin function at the edge does the I/O and matches on it.
- The kernel is synchronous. `async` belongs in the shells that own a socket or a pseudo-terminal. Never hold a lock across an `.await` (`await_holding_lock`), and never block inside async code.
- One task owns a piece of state and others send it messages. `Arc<Mutex<_>>` shared between tasks is the last resort, with the reason beside it.

## Errors

- `Result` is how Rust reports anything a caller could handle; a panic is for a bug. No `unwrap` outside tests (`unwrap_used`); an `expect` states the invariant that makes failure impossible.
- A library crate defines its errors as an enum, one variant per thing a caller can tell apart, with the cause kept as its source. Add context where the error crosses a boundary, never by turning it into a string.
- A public function that can fail documents how under `# Errors`, and one that can panic under `# Panics` (`missing_errors_doc`, `missing_panics_doc`).

## Unsafe and lints

- The kernel forbids `unsafe`. Elsewhere it lives in one small module per foreign interface, behind a safe API, and every block says why it is sound in a `// SAFETY:` comment (`undocumented_unsafe_blocks`).
- Silence a lint with `#[expect(..., reason = "...")]`, never `#[allow]`: it says why, and complains once it is no longer needed (`allow_attributes_without_reason`). Never loosen the workspace's lints to make a change pass.
- rustfmt's defaults and the Rust API Guidelines' naming outrank personal preference. Use what the 2024 edition gives: let chains, `let ... else` for an early return, `async fn` in traits without a helper crate.

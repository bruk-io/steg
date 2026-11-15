---
paths:
  - "crates/**/*.rs"
---
# Testing Rust in steg

Applies to Rust code and tests in crates/. The techniques are the repository's own `.claude/rules/testing-rust.md`; this is how to run them.

- `just test` runs the tests for the crates the change touches and every crate that depends on them. It chooses from the diff against `origin/main`, so fetch first and do not pick crates by hand. `just show-affected` prints what it chose.
- `just test-all` runs every crate. Use it when you doubt the selection, and say so in the evidence.
- `just check` is what a change must pass before it is called done. It also runs format, lint, boundaries and licences.
- A test may `unwrap` (`clippy.toml` allows it in tests); other code may not.
- When a step fails, the full output is in the log under `.quiet/` that the failure names. `STEG_VERBOSE=1` prints everything.

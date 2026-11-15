---
paths:
  - "crates/kernel/**"
---
# The kernel

Applies to code in crates/kernel. `crates/kernel/CLAUDE.md` has what this package is, its
dependencies and its tests; this file is the judgement `boundaries.toml` and the lint
(`.claude/rules/architecture.md`) can't make for you.

- An operation that needs the outside world - spawning a program, reading the clock - takes a
  trait or a callback the server supplies; the kernel never calls the real thing itself (R1).
  That is what lets its tests run with no doubles (`.claude/rules/testing-rust.md`) and is how
  the server stays the only owner of state.
- `crates/kernel/clippy.toml`'s disallowed lists are a curated, non-exhaustive set of `std`
  paths, not a provable total ban on I/O - don't go hunting for the one function that slipped
  through instead of trusting the mechanism; widen the list if a real gap turns up.

---
paths:
  - "crates/sdk/**"
---
# The sdk

Applies to code in crates/sdk. `crates/sdk/CLAUDE.md` has what this package is, its
dependencies and its tests; this file is the judgement `boundaries.toml` and the lint
(`.claude/rules/architecture.md`) can't make for you.

- The sdk stays usable from another language: no GPUI type and no particular async runtime
  appears in its public interface, and a required dependency is for data and serialisation
  only (R106). `boundaries.toml`'s empty allow list already refuses gpui, tokio and every other
  dependency as a crate; this is about what the sdk's own public types expose, which a
  dependency boundary can't see.
- The `local-socket` feature's provider (R103) is the one place in this crate allowed to do
  I/O. New code there, and its own test module (`crates/sdk/clippy.toml` is checked with
  `--all-features`, so the test module is linted too), carries its own
  `#[expect(clippy::disallowed_types, clippy::disallowed_methods, reason = "R103: ...")]`
  scoped to that module - nowhere else in the crate gets that expectation. An `#[expect]` that
  never fires is itself an error, so if the module stops doing real I/O this breaks the build
  and demands cleanup, rather than going stale.
- `crates/sdk/clippy.toml`'s disallowed lists are a curated, non-exhaustive set of `std` paths,
  not a provable total ban on I/O - don't go hunting for the one function that slipped through
  instead of trusting the mechanism; widen the list if a real gap turns up.

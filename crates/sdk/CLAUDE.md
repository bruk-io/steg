# steg-sdk

Steg's public interface (doc-16): the API description, the shapes that cross the API, and the
protocol's logic - encoding, decoding, the handshake and each stream's state.

- May depend on: nothing (`boundaries.toml`'s `[steg-sdk]` has an empty allow list).
- May not depend on: anything, steg's or external.
- Tests: `just test` runs this crate's tests when a change touches it; `just packages` builds
  and tests every package alone, including this one.
- Full spec: `backlog/docs/doc-16`.

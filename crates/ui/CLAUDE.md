# steg-ui

The app's components and the design's named values (doc-17): pure interface, with nothing
of steg's domain in it. Organised as foundations, atoms, molecules and organisms, by the
test each level sets (doc-17). Holds the gallery (R124), the program that shows every
component in every state, light and dark, and is the source of the image every UI pull
request shows.

- May depend on: `gpui`, and nothing of steg's (`boundaries.toml`'s `[steg-ui]`).
- May not depend on: `steg-sdk`, `steg-kernel`, `steg-server`, or `steg` - this package does
  no I/O and knows nothing of a session, tab, pane or client (R117, R118).
- A level uses only the levels before it: organisms use molecules, atoms and foundations;
  molecules use atoms and foundations; atoms use foundations; foundations use nothing of
  this crate (R119). Not yet a check `just check` runs - TASK-1.94 adds it.
- Every colour, size, radius, duration and font a *component* uses is a named value from
  foundations, never a literal (R120). The gallery is not a component of the four levels,
  but since TASK-1.93 it draws everything it shows - every swatch, ramp, type sample,
  spacing bar and corner - from `foundations` by name as well, the same as any real
  component would. Its own non-test code has no colour literal left: `gallery_theme`
  reaches the default session colour through `foundations::default_session`, not a hex.
  Two literals remain, both inside `#[cfg(test)]` in `gallery/main.rs` - `hex_of`'s own
  round-trip tests, which need a literal hex to assert against by the nature of what
  they prove. TASK-1.94's colour-literal check (once it lands) treats a `#[cfg(test)]`
  block the same as any other code, so whoever removes this crate's gallery-wide
  exemption for that check needs either a narrower carve-out for these two tests or to
  rehome them - flagged here rather than left for that task to discover.
- Tests: `just test` runs this crate's tests when a change touches it; `just check-packages`
  builds and tests every package alone, including this one (both its `src/` lib and its
  `gallery` binary - compiling the binary is cheap and proves nothing opens a window by
  itself; nothing in `cargo build`/`cargo test` ever executes it).
- The gallery: `just run-gallery` opens it; `STEG_GALLERY_THEME=dark just run-gallery` opens
  it in the other state; `t` toggles live. `just capture-gallery` captures both states to
  `target/gallery/{light,dark}.png` with no person at the screen - but it needs the
  invoking terminal to already hold a one-time, interactive Screen Recording permission
  grant (System Settings → Privacy & Security → Screen Recording), the same kind of
  one-time setup as installing Xcode's command-line tools. Without it, the recipe fails
  outright rather than attaching a blank or wrong image to a pull request. This recipe is
  never part of `just check`: GPUI 0.2.2 has no way to read a window's pixels except through
  OS-level screen capture, which GitHub's hosted macOS runner cannot grant (SIP-protected,
  re-imaged per job) - CI only ever compiles the gallery, never runs it.
- Full spec: `backlog/docs/doc-17`.

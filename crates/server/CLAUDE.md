# steg-server

The server (doc-16): the kernel plus sockets, pseudo-terminals, processes and the clock. One
process per person per machine.

- May depend on: `steg-kernel`, `steg-sdk` (`boundaries.toml`'s `[steg-server]`).
- May not depend on: `steg` or `steg-app`; no client reaches it except through the socket and
  the public API.
- Tests: `just test` runs this crate's tests when a change touches it; `just check-packages` builds
  and tests every package alone, including this one.
- Full spec: `backlog/docs/doc-16`.
- A pane's environment comes from a login shell, captured once at server start (`login_env.rs`,
  doc-8 R99) - never from the server process's own inherited environment. Accepted limit: the
  capture runs the shell as a login shell, not an interactive one, so on zsh only `.zprofile` is
  sourced, never `.zshrc` - a command pane's own program never sees anything set only there.

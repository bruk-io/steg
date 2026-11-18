---
paths:
  - "**/*.rs"
---
# Testing in Rust

Applies to Rust code and tests in steg. These are the techniques for the team's testing policy; the policy itself, and how to record evidence on a task, is the `team:testing` skill.

- Name a test by what it proves. Unit tests prove kernel logic and sit beside it in a `#[cfg(test)]` module, with no doubles. Functional tests prove behaviour across the server, with the outside world faked at the boundary, and sit in the crate's `tests/` folder. End-to-end tests run the real server, CLI and app.
- The outside world is the pseudo-terminal, the socket, the clock, the filesystem and the window. A fake is a small type implementing the same trait as the real thing and holding its state in plain fields; assert on that state, never on which calls were made.
- Do not use a mocking crate on code inside the boundary. A test that needs one is saying the decision is tangled with the I/O: move the decision into the kernel and test it there.
- Time, randomness and ids come in as parameters or through a trait. No test sleeps to wait for something; it waits on the event.
- A test must not rely on a dropped socket end making its peer's very next read or write fail at once: on macOS, `UnixStream::pair()`/`UnixListener::accept()` cannot set `CLOEXEC` atomically (the std library's own `sys/net/connection/socket/unix.rs`), so a `fork` elsewhere in the same test binary can inherit the dropped end before `CLOEXEC` lands, keeping the peer open and the expected failure from happening (TASK-1.113).
- A real temporary directory or a real socket in one is cheap and is not a double.
- A terminal behaviour is tested by feeding bytes in and reading the screen back, not by inspecting the emulator's internals.
- Run tests with `just test`, never `cargo test` directly: it runs only the crates the change affects and keeps the output short. `just check` is what a change must pass before it is called done.

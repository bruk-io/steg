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
- A real temporary directory or a real socket in one is cheap and is not a double.
- A terminal behaviour is tested by feeding bytes in and reading the screen back, not by inspecting the emulator's internals.
- Run tests through the repository's one check command, so the same thing runs by hand, in a worktree and in CI.

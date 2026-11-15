# bruk-io-steg

What agents need to know to work in [bruk-io/steg](https://github.com/bruk-io/steg), a terminal for macOS in Rust and GPUI with a server, a CLI and an app. Onboarded from commit 13f2998, when each crate was still a frame with no behaviour, and brought in line with the five packages of doc-16 after TASK-1.69.

The repository carries its own rules in `.claude/rules/` (architecture, Rust, testing, licence). The rules here add what depends on the layer being changed and how to run the checks.

- sdk (`crates/sdk`): the public interface, with no I/O of its own.
- kernel (`crates/kernel`): the server's state and its rules. Logic only.
- server (`crates/server`): the kernel plus the socket. Owns everything.
- app (`crates/app`): the GPUI window, a client.
- binary (`crates/steg`): the CLI and the server's entry point, the one program that ships.
- tools (`tools/`, `justfile`): the repository's own checks.

Commands are in `commands.toml`; all of them go through `just`.

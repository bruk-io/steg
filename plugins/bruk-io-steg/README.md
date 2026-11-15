# bruk-io-steg

What agents need to know to work in [bruk-io/steg](https://github.com/bruk-io/steg), a terminal for macOS in Rust and GPUI with a server, a CLI and an app. Onboarded from commit 13f2998, when each crate was still a frame with no behaviour.

The repository carries its own rules in `.claude/rules/` (architecture, Rust, testing, licence). The rules here add what depends on the layer being changed and how to run the checks.

- kernel (`crates/kernel`): state and its rules. Logic only, no dependencies.
- server (`crates/server`): the kernel plus the socket. Owns everything.
- cli (`crates/cli`): a client; its commands will come from the API description.
- app (`crates/app`): the GPUI window, a client.
- binary (`crates/steg`): puts the server and the CLI into the program that ships.
- tools (`tools/`, `justfile`): the repository's own checks.

Commands are in `commands.toml`; all of them go through `just`.

---
paths:
  - "**/*.rs"
  - "**/Cargo.toml"
---
# Licence

Applies to all Rust code and crate manifests in steg. Steg is MIT licensed and may be distributed later, so one copyleft file or dependency would have to be torn out again (R59).

- Never copy, port or closely adapt code from a GPL or AGPL source. Zed is the likely one: GPUI is permissively licensed, but Zed's terminal and editor crates are not, and they solve the same problems steg has. Read them to learn how GPUI works if needed; write steg's code fresh, from GPUI's own API.
- Before using code from any other project, read its licence file, not its README. Say on the task where the code came from and under what licence.
- A new dependency's licence is checked by the repository's check command, not by eye. If the check refuses a crate, choose another crate or raise it on the task; never add an exception to make it pass.
- Rex's documentation and its Program Status Protocol are a reference for behaviour. Implement from the published protocol text; take no code or assets from Rex.

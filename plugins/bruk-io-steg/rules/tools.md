---
layer: tools
paths:
  - "tools/**"
  - "justfile"
  - "Cargo.toml"
  - "boundaries.toml"
  - "deny.toml"
  - "clippy.toml"
---
# The repository's checks

Applies to the justfile, the workspace manifest, the check configuration and the Python tools in tools/.

- These are the checks every other change is judged by. Never loosen one to make a change pass: no licence added to `deny.toml`, no crate added to an allow list in `boundaries.toml`, and no lint relaxed in `Cargo.toml` or `clippy.toml`, unless that is the task and its plan says why.
- A recipe that runs a check sends its output through `steg-quiet`: one line when it passes, the end of its output and the path of the log when it fails. A new check recipe does the same.
- The tools use only the standard library, and each keeps its decision in a function that takes plain data, such as `selection` in `tools/src/steg_tools/affected.py`. Reading git and `cargo metadata` goes through `tools/src/steg_tools/workspace.py`.

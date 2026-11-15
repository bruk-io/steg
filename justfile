# The project's entrypoints. `just` lists them. Agents and people run these, never cargo directly,
# so the same thing runs by hand, in a task's worktree and in CI.
#
# Output is quiet: a step that passes prints one line. A step that fails prints the end of its
# output and the path of the full log. STEG_VERBOSE=1 shows everything.
#
# Until the Rust workspace exists, each step says so and passes. Once Cargo.toml is there, a
# step does its real work or fails; none passes by doing nothing.

set shell := ["bash", "-euo", "pipefail", "-c"]

rust := if path_exists("Cargo.toml") == "true" { "yes" } else { "no" }

# The tools are a small Python package in tools/, run through uv.
quiet := "uv run --quiet --project tools steg-quiet"
affected := "uv run --quiet --project tools steg-affected"

[private]
default:
    @just --list

# what a change must pass before it is called done: format, lint, boundaries, licences, the affected tests, the tools' own tests, the architecture model, and this file's own recipe names
check: check-fmt lint check-boundaries deny test test-tools check-architecture check-recipes

# the tests for the crates this change touches and the crates that depend on them (steg-affected chooses)
test:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "{{rust}}" = no ]; then echo "skip    tests: no Rust workspace yet"; exit 0; fi
    selection="$({{affected}})"
    case "$selection" in
        none) echo "skip    tests: this change touches no crate" ;;
        all)  {{quiet}} "tests (every crate)" -- cargo test --workspace ;;
        *)    {{quiet}} "tests ($selection)" -- cargo test $selection ;;
    esac

# every test in the workspace, whatever changed
test-all:
    @if [ "{{rust}}" = no ]; then echo "skip    tests: no Rust workspace yet"; else {{quiet}} "tests (every crate)" -- cargo test --workspace; fi

# the tests for the tools in tools/
test-tools:
    @{{quiet}} "tool tests" -- uv run --quiet --project tools pytest -q tools/tests

# build and test each package selected alone (`cargo -p`), so this is something anyone can run again
check-packages:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "{{rust}}" = no ]; then echo "skip    packages: no Rust workspace yet"; exit 0; fi
    for pkg in steg-sdk steg-kernel steg-server steg steg-app; do
        {{quiet}} "build ($pkg)" -- cargo build -p "$pkg"
        {{quiet}} "test ($pkg)" -- cargo test -p "$pkg"
    done

# format the code
fmt:
    @if [ "{{rust}}" = no ]; then echo "skip    format: no Rust workspace yet"; else {{quiet}} "format" -- cargo fmt --all; fi

# fail if the code is not formatted
check-fmt:
    @if [ "{{rust}}" = no ]; then echo "skip    format check: no Rust workspace yet"; else {{quiet}} "format check" -- cargo fmt --all --check; fi

# lints, with warnings as errors
lint:
    @if [ "{{rust}}" = no ]; then echo "skip    lint: no Rust workspace yet"; else {{quiet}} "lint" -- cargo clippy --workspace --all-targets -- -D warnings; fi

# refuse a dependency whose licence is not allowed (MIT project, nothing GPL or AGPL)
deny:
    @if [ "{{rust}}" = no ]; then echo "skip    licences: no Rust workspace yet"; else {{quiet}} "licences" -- cargo deny check licenses; fi

# refuse a crate dependency that crosses the boundaries in .claude/rules/architecture.md (boundaries.toml)
check-boundaries:
    @if [ "{{rust}}" = no ]; then echo "skip    boundaries: no Rust workspace yet"; else {{quiet}} "boundaries" -- uv run --quiet --project tools steg-boundaries; fi

# install or check what the recipes need beyond Rust itself (cargo-deny first). Safe to run again
setup:
    #!/usr/bin/env bash
    set -euo pipefail
    if command -v cargo-deny >/dev/null 2>&1; then
        echo "ok      cargo-deny already installed"
    else
        {{quiet}} "install cargo-deny" -- cargo install cargo-deny --locked
    fi

# build everything
build:
    @if [ "{{rust}}" = no ]; then echo "skip    build: no Rust workspace yet"; else {{quiet}} "build" -- cargo build --workspace; fi

# run the app
run:
    @if [ "{{rust}}" = no ]; then echo "skip    run: no Rust workspace yet"; else cargo run --package steg-app; fi

# which crates this change affects: all, none, or the -p flags `just test` passes to cargo
show-affected:
    @{{affected}}

# the architecture model (architecture/) validates against the LikeC4 grammar (doc-16, R108)
check-architecture:
    @{{quiet}} "architecture" -- npx --yes likec4@1.61.0 validate architecture

# a recipe's name starts with a verb on the list kept in tools/src/steg_tools/recipes.py (.claude/rules/just-recipes.md)
check-recipes:
    @{{quiet}} "recipe names" -- uv run --quiet --project tools steg-recipes

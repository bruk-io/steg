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

# mise.toml pins the Zig toolchain ghostty-vt-sys's build needs (TASK-1.34). Resolved once
# here, never required: if mise isn't installed, or hasn't installed this pin yet, this is
# the empty string, not a failure - `just` evaluates every backtick on every invocation
# (even `just fmt` or `just deny`, which never touch Zig), so this must never error on a
# machine with no mise at all. A recipe that actually compiles the workspace exports `ZIG`
# from it, conditionally (see `lint`/`test`/`test-all`/`packages`/`build`/`run` below); every
# other recipe is unaffected. Left unset rather than exported empty on purpose: an empty
# `ZIG` is worse than an unset one - `ghostty-vt-sys`'s own build.rs checks `ZIG` first and
# would try to run it verbatim, where an unset `ZIG` falls through to that build.rs's own
# fallback chain (a bare `zig` on PATH, then its own `mise which zig`, then a panic that says
# exactly what to install).
zig := `mise which zig 2>/dev/null || true`

[private]
default:
    @just --list

# what a change must pass before it is called done: format, lint, boundaries, licences, the affected tests, the tools' own tests, and the architecture model
check: fmt-check lint boundaries deny test test-tools architecture

# Seeds Zig's own package cache (`ZIG_GLOBAL_CACHE_DIR`, .cargo/config.toml) from the
# committed, read-only tarballs in third_party/ghostty-vt-zig-deps/p/, so a real build never
# reaches the network for them (third_party/README.md). `target/` is already gitignored and
# cargo-owned, so it's where Zig's own cache - fetched packages under `p/`, its compiled
# artifacts under `b/`/`h/`/`o/` - lives too; pointing `ZIG_GLOBAL_CACHE_DIR` straight at the
# tracked vendor directory instead filled it with megabytes of Zig's build cache on the first
# real build (found while building this task). `cp -n` is a no-op once seeded once.
[private]
zig-deps-seeded:
    @mkdir -p target/zig-global-cache/p && cp -n third_party/ghostty-vt-zig-deps/p/*.tar.gz target/zig-global-cache/p/ 2>/dev/null || true

# the tests for the crates this change touches and the crates that depend on them (steg-affected chooses)
test: zig-deps-seeded
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "{{rust}}" = no ]; then echo "skip    tests: no Rust workspace yet"; exit 0; fi
    if [ -n "{{zig}}" ]; then export ZIG="{{zig}}"; fi
    selection="$({{affected}})"
    case "$selection" in
        none) echo "skip    tests: this change touches no crate" ;;
        all)  {{quiet}} "tests (every crate)" -- cargo test --workspace ;;
        *)    {{quiet}} "tests ($selection)" -- cargo test $selection ;;
    esac

# every test in the workspace, whatever changed
test-all: zig-deps-seeded
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "{{rust}}" = no ]; then echo "skip    tests: no Rust workspace yet"; exit 0; fi
    if [ -n "{{zig}}" ]; then export ZIG="{{zig}}"; fi
    {{quiet}} "tests (every crate)" -- cargo test --workspace

# the tests for the tools in tools/
test-tools:
    @{{quiet}} "tool tests" -- uv run --quiet --project tools pytest -q tools/tests

# build and test each package selected alone (`cargo -p`), so this is something anyone can run again
packages: zig-deps-seeded
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "{{rust}}" = no ]; then echo "skip    packages: no Rust workspace yet"; exit 0; fi
    if [ -n "{{zig}}" ]; then export ZIG="{{zig}}"; fi
    for pkg in steg-sdk steg-kernel steg-server steg steg-app; do
        {{quiet}} "build ($pkg)" -- cargo build -p "$pkg"
        {{quiet}} "test ($pkg)" -- cargo test -p "$pkg"
    done

# format the code
fmt:
    @if [ "{{rust}}" = no ]; then echo "skip    format: no Rust workspace yet"; else {{quiet}} "format" -- cargo fmt --all; fi

# fail if the code is not formatted
fmt-check:
    @if [ "{{rust}}" = no ]; then echo "skip    format check: no Rust workspace yet"; else {{quiet}} "format check" -- cargo fmt --all --check; fi

# lints, with warnings as errors
lint: zig-deps-seeded
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "{{rust}}" = no ]; then echo "skip    lint: no Rust workspace yet"; exit 0; fi
    if [ -n "{{zig}}" ]; then export ZIG="{{zig}}"; fi
    {{quiet}} "lint" -- cargo clippy --workspace --all-targets -- -D warnings

# refuse a dependency whose licence is not allowed (MIT project, nothing GPL or AGPL)
deny:
    @if [ "{{rust}}" = no ]; then echo "skip    licences: no Rust workspace yet"; else {{quiet}} "licences" -- cargo deny check licenses; fi

# refuse a crate dependency that crosses the boundaries in .claude/rules/architecture.md (boundaries.toml)
boundaries:
    @if [ "{{rust}}" = no ]; then echo "skip    boundaries: no Rust workspace yet"; else {{quiet}} "boundaries" -- uv run --quiet --project tools steg-boundaries; fi

# install or check what the recipes need beyond Rust itself (cargo-deny first, then the
# pinned Zig mise.toml names - idempotent, and a no-op once it's installed)
setup:
    #!/usr/bin/env bash
    set -euo pipefail
    if command -v cargo-deny >/dev/null 2>&1; then
        echo "ok      cargo-deny already installed"
    else
        {{quiet}} "install cargo-deny" -- cargo install cargo-deny --locked
    fi
    if command -v mise >/dev/null 2>&1; then
        {{quiet}} "install the pinned Zig (mise.toml)" -- mise install
    else
        echo "skip    mise install: mise isn't on PATH - install it to build ghostty-vt (third_party/README.md)"
    fi

# build everything
build: zig-deps-seeded
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "{{rust}}" = no ]; then echo "skip    build: no Rust workspace yet"; exit 0; fi
    if [ -n "{{zig}}" ]; then export ZIG="{{zig}}"; fi
    {{quiet}} "build" -- cargo build --workspace

# run the app
run: zig-deps-seeded
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "{{rust}}" = no ]; then echo "skip    run: no Rust workspace yet"; exit 0; fi
    if [ -n "{{zig}}" ]; then export ZIG="{{zig}}"; fi
    cargo run --package steg-app

# which crates this change affects: all, none, or the -p flags `just test` passes to cargo
affected:
    @{{affected}}

# the architecture model (architecture/) validates against the LikeC4 grammar (doc-16, R108)
architecture:
    @{{quiet}} "architecture" -- npx --yes likec4@1.61.0 validate architecture

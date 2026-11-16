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
# from it, conditionally (see `lint`/`test`/`test-all`/`check-packages`/`build`/`run` below);
# every other recipe is unaffected. Left unset rather than exported empty on purpose: an
# empty `ZIG` is worse than an unset one - `ghostty-vt-sys`'s own build.rs checks `ZIG` first
# and would try to run it verbatim, where an unset `ZIG` falls through to that build.rs's own
# fallback chain (a bare `zig` on PATH, then its own `mise which zig`, then a panic that says
# exactly what to install).
zig := `mise which zig 2>/dev/null || true`

[private]
default:
    @just --list

# what a change must pass before it is called done: format, lint, boundaries, licences, the affected tests, every package alone, the tools' own tests, the architecture model, and this file's own recipe names
check: check-fmt lint check-boundaries deny test check-packages test-tools check-architecture check-recipes

# Seeds Zig's own package cache (`ZIG_GLOBAL_CACHE_DIR`, .cargo/config.toml) from the
# committed, read-only tarballs in third_party/ghostty-vt-zig-deps/p/, so a real build never
# reaches the network for them (third_party/README.md). `target/` is already gitignored and
# cargo-owned, so it's where Zig's own cache - fetched packages under `p/`, its compiled
# artifacts under `b/`/`h/`/`o/` - lives too; pointing `ZIG_GLOBAL_CACHE_DIR` straight at the
# tracked vendor directory instead filled it with megabytes of Zig's build cache on the first
# real build (found while building this task). `cp -n` is a no-op once seeded once.
[private]
zig-deps-seeded:
    #!/usr/bin/env bash
    # Copies, not `cp -n`: BSD cp (macOS) exits 1 when `-n` skips an already-present
    # file, which is the common case here (every run after the first) - `cp -n ...
    # || true` would make a *genuine* failure (missing source, unwritable target/)
    # silent too. Checking each destination ourselves tells the two apart.
    set -euo pipefail
    mkdir -p target/zig-global-cache/p
    shopt -s nullglob
    tarballs=(third_party/ghostty-vt-zig-deps/p/*.tar.gz)
    if [ ${#tarballs[@]} -eq 0 ]; then
        echo "error   zig-deps-seeded: no tarballs found under third_party/ghostty-vt-zig-deps/p/ (third_party/README.md)" >&2
        exit 1
    fi
    for f in "${tarballs[@]}"; do
        dest="target/zig-global-cache/p/$(basename "$f")"
        [ -e "$dest" ] && continue
        if ! cp "$f" "$dest"; then
            echo "error   zig-deps-seeded: failed to copy $f to $dest - check third_party/ghostty-vt-zig-deps/p/ is present and target/ is writable (third_party/README.md)" >&2
            exit 1
        fi
    done

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

# build and test each package alone (`cargo -p`, no `--all-features`, deliberately - see below): part of
# `check`, because a `--workspace` build unifies an optional feature on for every member that shares the
# dependency, even one whose own Cargo.toml never asked for it (doc-16 R105, R110); building each package
# by itself is the only way to prove the kernel never ends up able to compile against the sdk's optional
# parts, whatever some other member turns on in the same `--workspace` run. Also, with ghostty-vt now a
# real dependency (TASK-1.34), this is the step that pays Zig's build cost for steg-kernel alone - seeded
# from the vendored tarballs first so it never reaches the network.
#
# steg-sdk alone is also tested a second time, with `--features local-socket` turned on just
# for it: neither this loop's own default (no features) nor `lint`'s `--all-features` runs a
# test, so without this second run the local-socket provider's own tests (TASK-1.70) would
# never execute under `just check` at all - proving the package both with the feature off
# (the default every other consumer gets) and on (what its own tests need), each alone.
#
# steg-server alone is tested a second time the same way, with `--features race-fixture` -
# the same reason: off by default (a review of TASK-1.44 found the race test's own fixture
# binary should not build for an ordinary `cargo build`), so without this second run the
# race test (AC #2) would never execute under `just check` either.
#
# steg alone, the same way again, with `--features fixtures` (TASK-1.77): off by default for
# the identical reason - its two fixture binaries (the process-group-leader and autostart
# -client cases) should not build for an ordinary `cargo build -p steg`, so without this run
# `server_start.rs` (AC #1-#6) would never execute under `just check` either.
check-packages: zig-deps-seeded
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "{{rust}}" = no ]; then echo "skip    check-packages: no Rust workspace yet"; exit 0; fi
    if [ -n "{{zig}}" ]; then export ZIG="{{zig}}"; fi
    for pkg in steg-sdk steg-kernel steg-server steg steg-app; do
        {{quiet}} "build ($pkg)" -- cargo build -p "$pkg"
        {{quiet}} "test ($pkg)" -- cargo test -p "$pkg"
        if [ "$pkg" = steg-sdk ]; then
            {{quiet}} "test (steg-sdk, local-socket)" -- cargo test -p steg-sdk --features local-socket
        fi
        if [ "$pkg" = steg-server ]; then
            {{quiet}} "test (steg-server, race-fixture)" -- cargo test -p steg-server --features race-fixture
        fi
        if [ "$pkg" = steg ]; then
            {{quiet}} "test (steg, fixtures)" -- cargo test -p steg --features fixtures
        fi
    done

# format the code
fmt:
    @if [ "{{rust}}" = no ]; then echo "skip    format: no Rust workspace yet"; else {{quiet}} "format" -- cargo fmt --all; fi

# fail if the code is not formatted
check-fmt:
    @if [ "{{rust}}" = no ]; then echo "skip    format check: no Rust workspace yet"; else {{quiet}} "format check" -- cargo fmt --all --check; fi

# lints, with warnings as errors. --all-features so a feature-gated module (such as the sdk's
# local-socket provider, R103) is linted too, not silently skipped by every check that runs here.
# Seeded first (zig-deps-seeded): --all-features turns on steg-sdk's local-socket feature for
# everything in the --workspace build, which still links ghostty-vt through steg-kernel.
lint: zig-deps-seeded
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "{{rust}}" = no ]; then echo "skip    lint: no Rust workspace yet"; exit 0; fi
    if [ -n "{{zig}}" ]; then export ZIG="{{zig}}"; fi
    {{quiet}} "lint" -- cargo clippy --workspace --all-targets --all-features -- -D warnings

# refuse a dependency whose licence is not allowed (MIT project, nothing GPL or AGPL)
deny:
    @if [ "{{rust}}" = no ]; then echo "skip    licences: no Rust workspace yet"; else {{quiet}} "licences" -- cargo deny check licenses; fi

# refuse a crate dependency that crosses the boundaries in .claude/rules/architecture.md (boundaries.toml)
check-boundaries:
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
show-affected:
    @{{affected}}

# the architecture model (architecture/) validates against the LikeC4 grammar (doc-16, R108)
check-architecture:
    @{{quiet}} "architecture" -- npx --yes likec4@1.61.0 validate architecture

# a recipe's name starts with a verb on the list kept in tools/src/steg_tools/recipes.py (.claude/rules/just-recipes.md)
check-recipes:
    @{{quiet}} "recipe names" -- uv run --quiet --project tools steg-recipes

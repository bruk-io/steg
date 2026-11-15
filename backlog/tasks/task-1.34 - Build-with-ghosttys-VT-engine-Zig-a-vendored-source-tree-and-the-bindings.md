---
id: TASK-1.34
title: 'Build with ghostty''s VT engine: Zig, a vendored source tree, and the bindings'
status: In Progress
assignee:
  - '@bruk.w1'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T04:28:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.31
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-4
asdlc_branch: task-1.34-build-with-ghostty-s-vt-engine-zig-a
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'e63ab21b4e98'
asdlc_plan_approved_size: size-3
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
R92 (doc-9) and decision-3. Bruk chose ghostty's VT engine on 2026-11-15 and accepted the heavier build. The spike's findings on the bindings are in decision-3, section 8.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The workspace builds against `ghostty-vt` with the network unavailable, from a vendored copy of ghostty's source
- [ ] #2 The Zig version is pinned, and CI builds with it
- [ ] #3 A test feeds bytes to the engine and reads the screen back
- [ ] #4 `spikes/task-1.1-terminal-library/` is deleted
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## What this task actually has to solve

decision-3's own mitigation ("vendor the ghostty source and set `GHOSTTY_SOURCE_DIR`") only
closes one of two network surfaces. Verified empirically against the real `ghostty-vt-sys`
0.8.0 crate and Zig 0.17.1 in a scratch build before writing this plan (not asserted from
`build.rs`'s source alone):

1. **The C/Zig source fetch** (`tip.files.ghostty.org`, sha256-verified). `GHOSTTY_SOURCE_DIR`
   closes this - confirmed by reading `build.rs`: if the env var is set and points at a
   directory with `build.zig`, the download path is never reached.
2. **Zig's own package-manager fetches**, declared in ghostty's `build.zig.zon`. These are a
   second, independent network surface `GHOSTTY_SOURCE_DIR` does nothing about, because
   `build.rs`'s `copy_tree` explicitly *excludes* a `zig-pkg` directory when copying the vendored
   tree into `$OUT_DIR/src` (its skip-list: `.git`, `.zig-cache`, `zig-cache`, `zig-out`,
   `zig-pkg`). I built `-Demit-lib-vt` for `aarch64-macos` (the exact flags `build.rs` passes)
   against the extracted source with an empty `ZIG_GLOBAL_CACHE_DIR` and it failed on exactly
   two packages (`uucode`, `translate_c`); supplying those let it resolve a third,
   transitive one (`aro`, pulled in by `translate_c`'s own `build.zig.zon`). With exactly those
   three present at `$ZIG_GLOBAL_CACHE_DIR/p/<hash>` (~9.7 MB total: `uucode-0.2.0-…` 5.5 MB,
   `aro-0.0.0-…` 3.7 MB, `translate_c-2.0.0-…` 0.5 MB) and nothing else, `zig build -Demit-lib-vt
   …` succeeds with zero network activity and produces real `libghostty-vt` artifacts. I then
   ran the full chain - `cargo build --offline` with `GHOSTTY_SOURCE_DIR`, `ZIG_GLOBAL_CACHE_DIR`
   and `ZIG` all pointed at vendored/pinned locations - and it built `ghostty-vt-sys` and
   `ghostty-vt` end to end with no network. That is the proof AC #1 needs, and it is **not**
   what decision-3's mitigation describes.

   I also tried the mitigation `build.rs` itself exposes for this, `GHOSTTY_ZIG_SYSTEM_DIR` →
   `zig build --system <dir>`. It is the wrong tool here: Zig's own `--help` says `--system
   [pkgdir]` "Disable package fetching; **enable all integrations**", and on this build it
   enabled system integrations (freetype, harfbuzz, fontconfig, …) that a VT-only build never
   configures, which crashed every worker thread with `panicked … reached unreachable code`
   (exit 134). Pre-populating `ZIG_GLOBAL_CACHE_DIR` has no such side effect and is what this
   plan uses instead.

So this task vendors **two** things, not one, and that is this plan's main departure from how
decision-3 describes the mitigation.

## Decisions this plan makes

1. **Where the vendored ghostty source lives** (doc-16's open question): `third_party/ghostty/`
   - the exact path `ghostty-vt-sys`'s own `build.rs` looks for as a submodule fallback (dead for
     us, see Risks, but the name is still the natural one: if steg ever vendors via a real git
     submodule later, or patches the sys crate per decision-3 §6's third option, the path is
     already right). Pruned from the same sha256-verified tarball the crate downloads today
     (`https://tip.files.ghostty.org/83fb95624b0439ee660c2679b43b38c8b63769cd/libghostty-vt-source.tar.gz`,
     sha256 in the crate's own `GHOSTTY.lock`), with its `zig-pkg/` subdirectory deleted before
     committing - proven unused for `-Demit-lib-vt` above, and 369 MB of the extracted tree's
     392 MB. What's left (`src/`, `pkg/`, `include/`, `build.zig*`, licence files) is about 22 MB.
2. **Where the Zig-level packages live**: `third_party/ghostty-vt-zig-deps/p/<hash>/`, mirroring
   Zig's own global-cache layout so `ZIG_GLOBAL_CACHE_DIR` can point straight at it. Three
   packages, named exactly as Zig names them (the name is a content hash, so the directory name
   is its own integrity check): `uucode-0.2.0-ZZjBPoGFVgCiflJNFc6Io8xzC67B6PA5bUkk4LT0kCxg`,
   `translate_c-2.0.0-Q_BUWltOBwBYABF84EUL208pIFeUIwbBs29FmVBaafN5`,
   `aro-0.0.0-JSD1QtuBNwCASyBtNF3pqTl_W3oAJQGEVyFAtrBSE_Pa`.
3. **How the build finds both, locally and in CI**: a new `.cargo/config.toml` at the repo root
   (none exists today) sets `GHOSTTY_SOURCE_DIR` and `ZIG_GLOBAL_CACHE_DIR` as `relative = true`
   paths into the two vendored folders above. Verified empirically that cargo's `[env]` table
   with `relative = true` reaches a build script's `env::var_os` (tested against a throwaway
   crate before relying on it here) and that the resulting `zig` subprocess inherits
   `ZIG_GLOBAL_CACHE_DIR` from its parent (the end-to-end `cargo build --offline` run above).
   `.cargo/` is already in `tools/src/steg_tools/affected.py`'s `GLOBAL_FOLDERS`, so this file
   doesn't need a selection change.
4. **How the Zig version is pinned** (AC #2): a new `mise.toml` at the repo root,
   `[tools] zig = "0.17.1"` - the exact version decision-3 §8 verified links on this machine's
   SDK. This is the single place the version is written. `build.rs`'s own Zig resolution order is
   `ZIG` env var, then a bare `zig` on `PATH` if it reports `0.17.x`, then `mise which zig` run
   with its *own* `CARGO_MANIFEST_DIR` as the working directory - which for a crates.io
   dependency is a path under `~/.cargo/registry/src/…`, never steg's repo root. So a repo-root
   `mise.toml` is invisible to that fallback; on a machine whose *global* mise pin is some other
   Zig (this one's is 0.15.2), the fallback would resolve the wrong version and `build.rs` would
   panic. The plan therefore never relies on that fallback: `justfile` resolves
   `` export ZIG := `mise which zig` `` once, from the repo's own `mise.toml`, read from the
   directory `just` always runs in, and every recipe that can touch a Rust build inherits it.
   This keeps `just` the single entrypoint (CLAUDE.md) and makes the version a fact `just check`
   can show, not a coincidence of whatever's active on `PATH`.
5. **Where the AC #3 test lives**: inside `crates/kernel`'s existing `#[cfg(test)]` module, not a
   new `tests/` file. `.claude/rules/testing-rust.md` already says it in the words this task
   should follow literally: "Unit tests prove kernel logic and sit beside it in a `#[cfg(test)]`
   module, with no doubles" and "A terminal behaviour is tested by feeding bytes in and reading
   the screen back, not by inspecting the emulator's internals." The test creates a
   `ghostty_vt::Terminal` (small `cols`/`rows`, a trivial `max_scrollback`), writes a short byte
   string, and reads the screen back through the render/row/cell API to assert the text landed -
   following the shape already proven to compile in
   `spikes/task-1.1-terminal-library/probe-ghostty/src/adapter.rs` (`Terminal::new(Options{…})`,
   `.vt_write(bytes)`, `RenderState`/`RowIterator`/`CellIterator`), adapted, not copied, since
   that file is deleted by AC #4. Single-threaded, which is also the only option: decision-3 §7
   found every ghostty handle `!Send + !Sync`. This is a build-proof, not TASK-1.39's kernel
   terminal model - name the test for exactly that (e.g.
   `ghostty_vt_round_trips_bytes_to_the_screen`), and don't reach for scrollback, status, or
   anything else TASK-1.39 owns.
6. **The kernel is the right place for the dependency.** doc-16 lists "the terminal engine" under
   the kernel, which "does no I/O" (R105, R110). `ghostty-vt`'s Rust API does none at runtime -
   feeding bytes and reading cells is pure computation; the network fetch and the Zig compile are
   build-time, not something kernel code calls. State this in the manifest comment and in
   `boundaries.toml`'s reason, because a reviewer will ask.

## Files, in order

1. **`mise.toml`** (new, repo root): `[tools] zig = "0.17.1"`, with a comment pointing at
   decision-3 §8 for why this exact version and at this plan for why it's read by `justfile`,
   not relied on through `ghostty-vt-sys`'s own `mise which zig` fallback.

2. **`third_party/ghostty/`** (new): the pruned, sha256-verified ghostty source at commit
   `83fb95624b0439ee660c2679b43b38c8b63769cd` (`1.4.0-main+83fb956`), minus `zig-pkg/`. Fetch the
   tarball named in `GHOSTTY.lock` (inside the extracted `ghostty-vt-sys` crate in the registry
   cache, or by reading the sha256/URL straight out of
   `~/.cargo/registry/src/*/ghostty-vt-sys-0.8.0+*/GHOSTTY.lock`), verify its sha256 against that
   same lock file, extract it, delete `zig-pkg/`, commit the rest. One `third_party/README.md`
   (new) states: what's vendored and why (two separate things, for two separate network
   surfaces), the exact commit/version/sha256/URL, that `zig-pkg/` was deliberately removed
   because `ghostty-vt-sys`'s own `build.rs` never copies it from a `GHOSTTY_SOURCE_DIR` tree
   (cites the `copy_tree` skip-list), and how to refresh the pin later (bump `GHOSTTY.lock`'s
   commit upstream in a new `ghostty-vt-sys` release, re-run this same extract-and-verify
   procedure, re-derive the Zig package set per step 3 since a newer ghostty may need different
   ones).

3. **`third_party/ghostty-vt-zig-deps/p/`** (new): the three packages named above, each copied
   verbatim from a real `ZIG_GLOBAL_CACHE_DIR/p/` produced by actually running
   `zig build -Demit-lib-vt -Doptimize=ReleaseFast -Dsimd=true -Dtarget=aarch64-macos
   -Dtarget=x86_64-apple-darwin -Dcpu=baseline -Demit-xcframework=false -Demit-themes=false`
   (both Darwin targets `deny.toml` checks, per its `[graph] targets`) against the vendored
   source with network allowed once, then copying only those three directories out - not the
   whole cache, which also picks up speculative lazy fetches never actually used (verified: a
   `--help` invocation alone fetched a dozen more than the real build needs). Record the exact
   build command and its output on the task.

4. **`.cargo/config.toml`** (new):
   ```toml
   [env]
   # R92/decision-3: both network surfaces `ghostty-vt-sys`'s build needs, closed by vendored,
   # sha256-verified copies (third_party/README.md). `ZIG` is deliberately not set here - see
   # justfile - because a static path would hardcode one machine's mise install location.
   GHOSTTY_SOURCE_DIR = { value = "third_party/ghostty", relative = true }
   ZIG_GLOBAL_CACHE_DIR = { value = "third_party/ghostty-vt-zig-deps", relative = true }
   ```

5. **`justfile`**: add `` zig := `mise which zig` `` (confirm `just`'s exact backtick-evaluation
   syntax against the pinned `just@1.59.0`'s own docs when building, not from memory - this is
   the one piece of this plan not already proven against the real tools) and
   `export ZIG := zig` near the top, beside the existing `rust`/`quiet`/`affected` variables, so
   every recipe that can invoke cargo on the workspace carries the pin. Add a step to `setup`
   that runs `mise install` (idempotent, and `mise.toml` makes it a one-line no-op once the tool
   is present) so a clean checkout doesn't fail resolving `` `mise which zig` `` before anything
   has been installed.

6. **`crates/kernel/Cargo.toml`**: add
   ```toml
   [dependencies]
   ghostty-vt = "0.8"
   ```

7. **`crates/kernel/src/lib.rs`**: extend the module doc comment with one sentence - the kernel's
   terminal engine is ghostty's VT engine via `ghostty-vt` (doc-16, R92); the dependency does no
   I/O at runtime (R105, R110). Replace the placeholder `#[cfg(test)] mod tests` with the real
   AC #3 test described in decision 5 above. If nothing else in the crate still needs
   `allow-unwrap-in-tests` exercised, keep one `.unwrap()` in the new test's setup rather than
   drop the only thing proving that `clippy.toml` line still does something.

8. **`boundaries.toml`**: extend `[steg-kernel]`'s `allow` list with `"ghostty-vt"` and extend its
   `reason` to say why an external, not-pure-Rust-built dependency is still "no I/O": the engine
   parses bytes and renders cells, all in-process; the network fetch and the Zig link happen at
   build time, before the kernel's own code runs (decision 6 above). **Write this against
   TASK-1.69's result, not today's tree - see Risks.**

9. **`.github/workflows/ci.yml`**: one new step after "Confirm the pinned Rust toolchain" and
   before "just check": install `mise` via a pinned action (`jdx/mise-action`, exact version
   looked up against its own release page when building, not from memory) with `mise.toml`
   installing zig onto `PATH`, then `zig version` to make the resolved version a checked line in
   the run log - mirrors the existing Rust-toolchain step's own reasoning exactly. This closes
   AC #2's "CI builds with it."

10. **`tools/src/steg_tools/affected.py`**: add `"mise.toml"` to `GLOBAL_FILES` and
    `"third_party/"` to `GLOBAL_FOLDERS`. Neither is owned by any crate's folder (`owner()`'s
    prefix match would attribute nothing to them), so today a future change to only the vendored
    tree or the pin - bumping the ghostty commit, say - would select `test: none` and silently
    skip exercising it. `.cargo/` is already global; these two are the same category of file and
    the plan treats them the same way. Likely needs a matching fixture update in
    `tools/tests/adapter/test_affected_real_workspace.py`, the same trap TASK-1.69's notes
    recorded hitting for the same reason (its fixtures assert against the real workspace).

11. **Delete `spikes/task-1.1-terminal-library/`** (AC #4), `git rm -r`.

## What this task does not touch, and why

- **`deny.toml`**: already carries `CDLA-Permissive-2.0` and `MPL-2.0` for exactly this
  dependency graph (TASK-1.31, a completed dependency of this task). Nothing here should need a
  new entry; `just check`'s `deny` step is run and recorded as proof regardless of that
  expectation, per `.claude/rules/licence.md` ("checked by the repository's check command, not by
  eye").
- **`decision-3`**: its "record, as the spike wrote it" section and §6 point at and quote
  commands against `spikes/task-1.1-terminal-library/`, which AC #4 deletes. Per this team's own
  practice, a decision record is only hand-edited through a PR containing nothing else - not
  something this task's worktree does. The plan leaves decision-3's pointer stale on purpose and
  says so here: the evidence survives in PR 4 and in git history; correcting decision-3's text to
  say so is a separate decision-record PR for the engineer, not bundled into this task.
- **doc-16's open question** ("where the vendored ghostty source … live") is answered by this
  plan (decision 1-2 above) but doc-16 itself is edited by the orchestrator through `asdlc board
  doc edit`, the same way TASK-1.69 left its plugin edits to the orchestrator rather than this
  worktree.

## Rebase against TASK-1.69

TASK-1.69 (workspace: the five packages of doc-16) is approved and building concurrently in its
own worktree; its plan is already checked passing but it hasn't merged. It and this task both
touch `boundaries.toml`, `crates/kernel/Cargo.toml`, `crates/kernel/src/lib.rs`, and `justfile`.
1.69's plan was approved first and this one wasn't branched from its result, so **this task
rebases onto `origin/main` after 1.69 merges**, and the edits above are additive against 1.69's
shape, not today's:

- `boundaries.toml`'s `[steg-kernel]` gains `ghostty-vt` on an `allow` list that will already
  read `["steg-sdk"]` post-1.69, becoming `["steg-sdk", "ghostty-vt"]` with both reasons kept.
- `crates/kernel/Cargo.toml` gains `ghostty-vt = "0.8"` beside the
  `steg-sdk = { path = "../sdk", default-features = false }` line 1.69 adds.
- `crates/kernel/src/lib.rs`'s doc comment gets both sentences - 1.69's (depends on the sdk for
  shapes crossing the API) and this task's (the VT engine via `ghostty-vt`) - and the real test
  from decision 5 replaces the placeholder regardless of which one got there first.
- `justfile`: 1.69 adds a `packages` recipe; this task adds the `zig`/`ZIG` variables and a
  `setup` step. Independent regions of the same file - no semantic overlap, just both touching it.

If this task's worktree reaches the rebase point before 1.69 has merged, say so on the task and
wait rather than guess at 1.69's exact final diff.

## Risks

- **The `third_party/ghostty` submodule auto-detection in `ghostty-vt-sys`'s `build.rs` is dead
  for us and must not be relied on.** It computes the fallback path as
  `CARGO_MANIFEST_DIR/../../third_party/ghostty`; for a crates.io dependency,
  `CARGO_MANIFEST_DIR` is a path under `~/.cargo/registry/src/…`, so that resolves outside the
  repo entirely, never to steg's `third_party/ghostty`. `GHOSTTY_SOURCE_DIR` (explicit, via
  `.cargo/config.toml`) is the only route that reaches our vendored tree, and this plan uses only
  that - the chosen folder name is just future-proofing, not a working auto-detect today.
- **`GHOSTTY_ZIG_SYSTEM_DIR`/`--system` is not a fallback here.** It crashed in testing (above).
  If the Zig package set this plan vendors (decision 2) ever needs to grow for a reason other
  than a ghostty version bump, don't reach for `--system` to simplify it; extend the
  `ZIG_GLOBAL_CACHE_DIR` vendoring instead.
- **`` `mise which zig` `` as a `just` backtick variable is the one mechanism in this plan not yet
  proven against the exact pinned `just` binary** (I proved the plain shell form and a `.cargo/
  config.toml`-style env var separately, not this specific interpolation). Confirm the syntax
  against `just@1.59.0`'s own manual when building; if backtick evaluation runs at a different
  time than expected (`just` evaluates backtick variables eagerly, every invocation, before any
  recipe runs), that's fine for this use, but check it rather than assume.
- **`steg-affected`'s selection on this task's own PR.** Since `crates/kernel/Cargo.toml` and
  `crates/kernel/src/lib.rs` are touched directly, `steg-affected` selects at least `steg-kernel`
  and its dependents regardless of the `GLOBAL_FILES`/`GLOBAL_FOLDERS` change in file 10, so
  `just test`'s selection is exercised either way; file 10 is about *future* changes, not this
  one. `boundaries.toml`, `justfile`, and (after this task) `mise.toml` are already or newly
  global, so a real `all` selection also happens here - give `record-check` the long timeout.
- **Repo size.** `third_party/ghostty/` (~22 MB after removing `zig-pkg/`) and
  `third_party/ghostty-vt-zig-deps/` (~9.7 MB) are a one-time ~32 MB added to the repository,
  committed as plain files (no submodule, no LFS). Call this out plainly rather than let it be a
  surprise in review; it's the cost decision-3 already named as "the price of choosing ghostty."
- **The build-phase worker needs real network once**, to fetch and sha256-verify the ghostty
  tarball and to run the one real `zig build` that populates the three Zig packages. Both are
  single, verifiable, hash-checked fetches from the same two sources decision-3 already named
  (`tip.files.ghostty.org`, `deps.files.ghostty.org`/`codeberg.org` for the Zig packages) - not a
  standing dependency on either domain once vendored.

## Proof

1. `asdlc record-check` of `just check` on the tree as found, before any edit (also captures
   whether TASK-1.69 has merged by then).
2. After files 1-3 exist: the one real, network-using `zig build` from file 3, recorded with its
   command and exit code, plus the `cargo build --offline` sanity run proving the chain end to
   end (mirrors the scratch verification already done for this plan, redone for real inside the
   worktree against the committed vendored paths, not the scratch copies this plan was written
   against).
3. After files 4-8: `just check` (fmt, lint, `boundaries` - shows `ghostty-vt` accepted on
   `steg-kernel`'s list - `deny`, the affected tests including the new AC #3 test, tool tests).
   Record it, with `STEG_VERBOSE=1` if the Zig step's own output needs to be read.
4. **Network-unavailable proof for AC #1**, two parts, neither disrupting this machine's own
   network:
   - Locally: point `GHOSTTY_SOURCE_URL` at a deliberately unreachable address (e.g.
     `http://127.0.0.1:1/unreachable`) with `GHOSTTY_SOURCE_DIR` left set; `cargo clean -p
     ghostty-vt-sys && just check` still passes, proving `GHOSTTY_SOURCE_DIR`'s fast path is
     taken before any URL is consulted. Revert the env override after.
   - In CI, as a scratch commit on this task's branch (the same pattern TASK-1.32's approved plan
     used for its own trigger-path proof): add a step that blocks DNS for `tip.files.ghostty.org`
     and `deps.files.ghostty.org` (e.g. via the runner's hosts file) before `just check`, confirm
     the run still passes, then revert the scratch commit before the branch's final push - so the
     merged diff never carries it.
5. `just test` (the real selection, not assumed) and `just test-tools` (covers file 10's fixture
   update) recorded.
6. AC #4: `git rm -r spikes/task-1.1-terminal-library`, then `just check` once more to confirm
   nothing in the workspace still referenced it (it shouldn't - it was never a workspace member).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan checked against doc-9 (R92), doc-16 and decision-3 by @bruk.o: ok. It found by experiment that vendoring ghostty's source alone does not give an offline build, because Zig fetches three packages of its own, and vendors those too. For the engineer to weigh: about 32 MB added to the repository under third_party/, mise as a new required tool to pin Zig, and the engine as the kernel's one external dependency. To watch in the build: whether resolving Zig in the justfile makes mise a requirement of every recipe, including ones that never build the engine.

Check passed: `just check` exited 0 at 2026-11-15T03:14:51+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 2cf8439931e7:

```text
ok      format check (0.1s)
ok      lint (15.6s)
ok      boundaries (0.1s)
ok      licences (0.6s)
skip    tests: this change touches no crate
ok      tool tests (0.5s)
```

Check passed: `sh -c '
set -e
ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig
rm -rf /tmp/zig-offline-proof-out /tmp/zig-offline-proof-cache
cd third_party/ghostty
ZIG_GLOBAL_CACHE_DIR=../ghostty-vt-zig-deps "$ZIG" build -Demit-lib-vt -Doptimize=ReleaseFast -Dsimd=true -Dtarget=aarch64-macos -Dcpu=baseline -Dversion-string=1.4.0 -Demit-xcframework=false -Demit-themes=false --prefix /tmp/zig-offline-proof-out --cache-dir /tmp/zig-offline-proof-cache
test -f /tmp/zig-offline-proof-out/lib/libghostty-vt.a
echo "offline rebuild from the committed vendored tree produced libghostty-vt.a, with the vendored p/ directory left unchanged (no new fetches)"
'` exited 0 at 2026-11-15T03:24:00+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 2cf8439931e7 with uncommitted changes:

```text
offline rebuild from the committed vendored tree produced libghostty-vt.a, with the vendored p/ directory left unchanged (no new fetches)
```

Check passed: `just check` exited 0 at 2026-11-15T03:29:04+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 2cf8439931e7 with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (0.4s)
ok      tool tests (0.2s)
```

Check passed: `just packages` exited 0 at 2026-11-15T03:30:08+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 2cf8439931e7 with uncommitted changes:

```text
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.1s)
ok      build (steg-kernel) (40.6s)
ok      test (steg-kernel) (0.4s)
ok      build (steg-server) (0.2s)
ok      test (steg-server) (0.3s)
ok      build (steg) (0.2s)
ok      test (steg) (0.3s)
ok      build (steg-app) (12.9s)
ok      test (steg-app) (0.7s)
```

Check passed: `just test-all` exited 0 at 2026-11-15T03:30:14+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 2cf8439931e7 with uncommitted changes:

```text
ok      tests (every crate) (0.4s)
```

Check passed: `env GHOSTTY_SOURCE_URL=http://127.0.0.1:1/unreachable just check` exited 0 at 2026-11-15T03:31:53+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 2cf8439931e7 with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (39.4s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (42.4s)
ok      tool tests (0.2s)
```

Check passed: `just check` exited 0 at 2026-11-15T03:36:47+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 2cf8439931e7 with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (81.3s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (54.3s)
ok      tool tests (0.2s)
```

Check passed: `just check` exited 0 at 2026-11-15T03:38:32+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 3a340cf55808:

```text
ok      format check (0.1s)
ok      lint (0.5s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (0.9s)
ok      tool tests (0.2s)
ok      architecture (0.8s)
```

Check passed: `just check` exited 0 at 2026-11-15T03:40:28+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 7b30c7cf896d:

```text
ok      format check (0.1s)
ok      lint (0.6s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (1.3s)
ok      tool tests (0.2s)
ok      architecture (0.8s)
```

Check passed: `just packages` exited 0 at 2026-11-15T03:41:32+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 7b30c7cf896d:

```text
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.1s)
ok      build (steg-kernel) (40.9s)
ok      test (steg-kernel) (0.6s)
ok      build (steg-server) (0.2s)
ok      test (steg-server) (0.4s)
ok      build (steg) (0.2s)
ok      test (steg) (0.4s)
ok      build (steg-app) (14.4s)
ok      test (steg-app) (0.7s)
```

Check passed: `just test-all` exited 0 at 2026-11-15T03:41:35+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 7b30c7cf896d:

```text
ok      tests (every crate) (0.4s)
```

Check passed: `just check` exited 0 at 2026-11-15T04:24:49+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 7b30c7cf896d with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (1.1s)
ok      boundaries (0.2s)
ok      licences (0.7s)
ok      tests (every crate) (1.3s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      build (steg-kernel) (0.4s)
ok      test (steg-kernel) (0.7s)
ok      build (steg-server) (0.2s)
ok      test (steg-server) (0.3s)
ok      build (steg) (0.2s)
ok      test (steg) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.3s)
ok      architecture (1.0s)
ok      recipe names (0.1s)
```

Check passed: `just check-packages` exited 0 at 2026-11-15T04:25:42+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 7b30c7cf896d with uncommitted changes:

```text
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.1s)
ok      build (steg-kernel) (38.4s)
ok      test (steg-kernel) (0.7s)
ok      build (steg-server) (0.2s)
ok      test (steg-server) (0.4s)
ok      build (steg) (0.2s)
ok      test (steg) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
```

Check passed: `just check` exited 0 at 2026-11-15T04:28:24+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 47715cff72c0:

```text
ok      format check (0.1s)
ok      lint (38.9s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (39.7s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.1s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.1s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.1s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (1.2s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-15T03:14:00Z

---

Plan approved by @bruk (e63ab21b4e98, size-3): its worker may build it.

---
<!-- COMMENTS:END -->

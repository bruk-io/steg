---
id: TASK-1.34
title: 'Build with ghostty''s VT engine: Zig, a vendored source tree, and the bindings'
status: In Review
assignee:
  - '@bruk.w1'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T07:02:00Z'
labels:
  - size-3
  - review-passed
milestone: m-0
dependencies:
  - TASK-1.31
references:
  - task-1.34-build-with-ghostty-s-vt-engine-zig-a
  - https://github.com/bruk-io/steg/pull/20
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-4
asdlc_branch: task-1.34-build-with-ghostty-s-vt-engine-zig-a
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'e63ab21b4e98'
asdlc_plan_approved_size: size-3
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=0,nit=1'
asdlc_plan_matched: no
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

Check passed: `env GHOSTTY_SOURCE_URL=http://127.0.0.1:1/unreachable just check` exited 0 at 2026-11-15T04:30:40+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 47715cff72c0:

```text
ok      format check (0.1s)
ok      lint (39.5s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (39.8s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.1s)
ok      build (steg-kernel) (38.5s)
ok      test (steg-kernel) (0.5s)
ok      build (steg-server) (0.2s)
ok      test (steg-server) (0.3s)
ok      build (steg) (0.2s)
ok      test (steg) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.1s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```

Check passed: `just check` exited 0 at 2026-11-15T04:39:11+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at dad117921212:

```text
ok      format check (0.1s)
ok      lint (28.7s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (29.3s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.1s)
ok      build (steg-kernel) (28.4s)
ok      test (steg-kernel) (0.5s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.3s)
ok      build (steg) (0.2s)
ok      test (steg) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.2s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```

Check passed: `just check` exited 0 at 2026-11-15T04:42:53+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 043be079b8f7:

```text
ok      format check (0.1s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (0.4s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.1s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.1s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.1s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (0.9s)
ok      architecture (1.1s)
ok      recipe names (0.0s)
```

Built as planned, with departures found while building (none change the plan's shape, all
recorded here as the plan instructs for a small departure):

## What was built

- `third_party/ghostty/`: ghostty's own C/Zig source at commit `83fb95624b0439ee660c2679b43b38c8b63769cd`
  (`1.4.0-main+83fb956`), fetched from the URL in `ghostty-vt-sys`'s own `GHOSTTY.lock` and
  sha256-verified against that same lock file before extracting. `GHOSTTY_SOURCE_DIR`
  (`.cargo/config.toml`) points at it.
- `third_party/ghostty-vt-zig-deps/p/`: seven tarballs (not three - see Departures) Zig's
  own package manager needs to resolve `ghostty/build.zig.zon` for `-Demit-lib-vt` on both
  Darwin targets, found by building against an empty cache and reading which packages it
  asked for.
- `mise.toml`: pins Zig `0.17.1`.
- `.cargo/config.toml`: `GHOSTTY_SOURCE_DIR` and `ZIG_GLOBAL_CACHE_DIR` as relative env vars.
- `justfile`: a `zig` variable resolving `mise which zig`, exported as `ZIG` conditionally in
  every recipe that can trigger a real compile (`lint`, `test`, `test-all`, `check-packages`,
  `build`, `run`), plus a private `zig-deps-seeded` recipe those all depend on.
- `boundaries.toml`: `ghostty-vt` added to `[steg-kernel]`'s `allow` list, reason extended.
- `crates/kernel/Cargo.toml`: `ghostty-vt = "0.8"`.
- `crates/kernel/src/lib.rs`: a `#[cfg(test)]` module with
  `ghostty_vt_round_trips_bytes_to_the_screen` (AC #3) - feeds `b"hello"` to a real
  `ghostty_vt::Terminal` and reads it back through the render/row/cell API.
- `crates/kernel/CLAUDE.md`: corrected to say the kernel may depend on `ghostty-vt`.
- `.github/workflows/ci.yml`: installs the pinned Zig via `jdx/mise-action` (real latest
  release looked up against GitHub's API when writing this, not from memory: `v5.2.8`,
  `d5574b52816a478f9d4e6529a3f6ff17a8a36785`) and confirms the version in the log.
- `tools/src/steg_tools/affected.py`: `mise.toml` and `third_party/` join the global
  file/folder list.
- Deleted `spikes/task-1.1-terminal-library/` (AC #4).

## Departures from the plan, found while building

1. **Two network surfaces turned out to need separate handling from what the plan described,
   and the actual Zig package set is seven tarballs, not three.** The plan's own "what this
   task actually has to solve" section already knew `GHOSTTY_SOURCE_DIR` alone wasn't enough
   and planned to vendor Zig's own package fetches too; building it found the real,
   complete set is larger: `uucode`, `translate_c`, `aro` (the three named in the plan) plus
   `zlib`, `pixels`, `highway`, `wuffs` (four more, found when building the real
   `aarch64-apple-darwin` *and* `x86_64-apple-darwin` targets together - a `--help` probe or a
   single-target build undercounts this). All seven are named with what they are and why in
   `third_party/README.md`.
2. **`ZIG_GLOBAL_CACHE_DIR` cannot point at the committed vendor directory directly.** The
   plan's file list implied it could. Building it found Zig writes its own compiled-artifact
   cache (`b/`, `h/`, `o/`, `z/`, `tmp/`) into whatever directory `ZIG_GLOBAL_CACHE_DIR` names,
   alongside the fetched packages under `p/` - pointing it straight at
   `third_party/ghostty-vt-zig-deps` filled that tracked directory with ~60 MB of Zig's own
   build cache on the first real `just check`. Fixed before anything was committed:
   `ZIG_GLOBAL_CACHE_DIR` now points at the gitignored `target/zig-global-cache`, and a private
   `zig-deps-seeded` recipe copies the vendored tarballs there before any recipe that can
   trigger a real build. `third_party/ghostty-vt-zig-deps/p/` itself stays exactly the seven
   tarballs, nothing else.
3. **`justfile`'s `ZIG` export is per-recipe and conditional, not the blanket top-level
   `export ZIG := zig` the plan's decision 4 described**, because of the plan's own flagged
   risk: `just` evaluates a top-level backtick on every invocation, even `just fmt` or
   `just deny`, which never touch Zig. Verified (not assumed): with no `mise` at all on `PATH`,
   a bare top-level `` zig := `mise which zig` `` makes `just` fail *every* recipe with an
   opaque `backtick failed with exit code 127` - mise becomes a hard requirement of the whole
   justfile, and the failure doesn't say what to install. The built version resolves
   `` zig := `mise which zig 2>/dev/null || true` `` (never fails) once, and only `lint`,
   `test`, `test-all`, `check-packages`, `build` and `run` conditionally `export ZIG="{{zig}}"`
   when it resolved to something - every other recipe is unaffected by mise's presence or
   absence, and a machine with no mise falls through to `ghostty-vt-sys`'s own build.rs
   fallback chain and its already-clear panic message ("Install it with `mise install`...").
   An empty `ZIG` was deliberately not exported either: verified it produces a worse message
   than an unset one (`env::var_os("ZIG")` returns `Some("")` for a set-but-empty var, so
   `check_zig_version` runs `Command::new("").arg("version")`, a confusing OS error, instead of
   falling through to build.rs's own chain).
4. **`crates/kernel/src/lib.rs`'s placeholder test is gone**, rewritten by TASK-1.35 before
   this task's plan's "replace the placeholder" step could apply - TASK-1.35 merged into main
   mid-build, and 1.34's branch picked up real kernel modules (`colour`, `error`, `ids`,
   `pane`, `session`, `tab`, `tree`) with their own per-module tests. The AC #3 test lands as
   its own `#[cfg(test)] mod tests` at the crate root instead, next to the module doc comment's
   new sentence about the engine - there was no placeholder left to replace.
5. **`boundaries.toml`'s `[steg-kernel]` also merged in a `deny-features` key** (added by
   another task while this one was building) alongside the `allow` list this task extends;
   both are kept. **`justfile`'s recipes were renamed** (`packages` -> `check-packages`,
   `fmt-check` -> `check-fmt`, `boundaries` -> `check-boundaries`, `affected` -> `show-affected`)
   and gained `check-architecture`/`check-recipes` (a rule that a recipe's name starts with a
   verb) while this task was building; the private `zig-deps-seeded` recipe is exempt from that
   rule (private recipes are, by the rule's own text - "nobody types it by name") and needed no
   rename. `lint` also gained `--all-features` from the same source; this task's `ZIG` export
   and `zig-deps-seeded` dependency were carried onto the renamed/expanded recipe bodies rather
   than reapplied against the old names.
6. **Build-time cost, since the kernel now links the engine**: from a clean `target/`,
   `check-packages`'s `build (steg-kernel)` step (steg-kernel built alone, no feature
   unification from siblings) costs ~28-40s (compiling `ghostty-vt-sys`'s Zig build,
   `ghostty-vt`, then `steg-kernel`), where it was near-instant before. `lint`'s
   `--workspace --all-features` clippy pass and `test`'s full-workspace build pay the same Zig
   link once per clean `target/`, then it's cached (confirmed: a second `just check` with a
   warm `target/` finished the same steps in well under a second each).
7. **The network-block proof for AC #1 ran as a throwaway PR on its own branch, not a scratch
   commit on this branch.** The standing rule never force-pushes, and TASK-1.32's precedent's
   "revert the scratch commit... so the merged diff is only..." only works without a force-push
   if the scratch commit never reaches the real branch in the first place. So: a separate
   branch/worktree/PR (`task-1.34-scratch-network-proof`, PR #24) added one step to
   `.github/workflows/ci.yml` that blocks `tip.files.ghostty.org`, `deps.files.ghostty.org` and
   `codeberg.org` via `/etc/hosts` on that run's ephemeral runner only, pushed it, watched the
   run pass, then closed the PR without merging and deleted the branch. Nothing from it ever
   touched `task-1.34-build-with-ghostty-s-vt-engine-zig-a`.
8. **Checked `deny.toml`, as the plan expected not to need it**: unchanged. TASK-1.31 already
   landed the `CDLA-Permissive-2.0`/`MPL-2.0` allowances this dependency graph needs; `just
   check-boundaries`... `check-fmt`... `deny` all passed clean in every recorded run below.
9. **doc-16's open question and decision-3's now-stale pointers are left exactly as the plan
   said**: not edited in this worktree. doc-16's "where the vendored ghostty source...live" is
   answered by `third_party/README.md`'s own account, for the orchestrator to carry into the
   doc through `asdlc board doc edit`. decision-3's "record, as the spike wrote it" section and
   its `cargo tree` commands in section 6 point at `spikes/task-1.1-terminal-library/`, which
   AC #4 deletes - its evidence survives in PR 4 and git history; correcting decision-3's text
   is a separate decision-record PR, per the team's practice of hand-editing a decision record
   only through a PR with nothing else in it.

## Which recorded run proves which acceptance criterion

- **AC #1** (builds against `ghostty-vt` with the network unavailable, from a vendored copy):
  - Local: `env GHOSTTY_SOURCE_URL=http://127.0.0.1:1/unreachable just check`, recorded twice
    (pre- and post-merge), both after a full `cargo clean -p ghostty-vt-sys` - proves
    `GHOSTTY_SOURCE_DIR`'s fast path is taken before any URL is ever consulted.
  - GitHub (can't run inside the worktree, reported here per the task's own instructions):
    scratch PR #24, run **43677134619**, `tip.files.ghostty.org`/`deps.files.ghostty.org`/
    `codeberg.org` blocked on the runner via `/etc/hosts`, `just check` passed in 5m21s. Closed
    without merging; branch deleted.
  - The real PR's own runs are a second, independent confirmation: a fresh macOS runner has no
    pre-existing Zig or Cargo registry cache for this new dependency, so a green run there
    (**43673513113**, **43677712214**) depends on the vendored content and nothing else.
- **AC #2** (Zig version pinned, CI builds with it): `mise.toml` pins `0.17.1`; every recorded
  `just check` below built with it (`justfile`'s `zig`/`ZIG` resolution); CI's "Confirm the
  pinned Zig toolchain" step prints `0.17.1` in every PR run's log (runs above).
- **AC #3** (a test feeds bytes to the engine and reads the screen back):
  `ghostty_vt_round_trips_bytes_to_the_screen` in `crates/kernel/src/lib.rs`, passing in every
  `just check`/`just check-packages` run recorded below.
- **AC #4** (the spike is deleted): `git rm -r spikes/task-1.1-terminal-library`, in the first
  real commit on this branch; `just check` has passed on every run since, with the spike gone.

## Checks recorded on this task (asdlc record-check), in order

1. `just check` - baseline, before any edit.
2. The real `zig build -Demit-lib-vt` populating `third_party/ghostty-vt-zig-deps/p/`, and an
   offline rebuild against the committed vendored tree - both pass.
3. `just check`, `just packages` (pre-rename), `just test-all` - all pass after the first real
   commit.
4. `env GHOSTTY_SOURCE_URL=http://127.0.0.1:1/unreachable just check` - passes (AC #1, local
   half).
5. After merging TASK-1.35 (lib.rs conflict resolved) and the later main (boundaries.toml's
   `deny-features`, justfile's recipe renames and `check-architecture`/`check-recipes`,
   resolved): `just check` passes, including the renamed `check-packages` and the new
   `check-recipes`/`check-architecture` steps.
6. `env GHOSTTY_SOURCE_URL=... just check` repeated against the final merged state - passes.
7. `just check` repeated after each of two further main-only (board-file) merges - passes each
   time, most recently from a warm `target/` (every step under 1.3s, confirming the Zig link
   is cached once paid).

PR: https://github.com/bruk-io/steg/pull/20, branch
`task-1.34-build-with-ghostty-s-vt-engine-zig-a`, up to date with `origin/main` as of this
note.

Check passed: `sh -c '
set -e
ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig
rm -rf /tmp/ghostty-refresh-out /tmp/ghostty-refresh-cache /tmp/ghostty-refresh-pkg
cd third_party/ghostty
for t in aarch64-macos x86_64-macos; do
  ZIG_GLOBAL_CACHE_DIR=../ghostty-vt-zig-deps ZIG_LOCAL_PKG_DIR=/tmp/ghostty-refresh-pkg "$ZIG" build -Demit-lib-vt -Doptimize=ReleaseFast -Dsimd=true -Dtarget=$t -Dcpu=baseline -Demit-xcframework=false -Demit-themes=false --prefix /tmp/ghostty-refresh-out --cache-dir /tmp/ghostty-refresh-cache
done
test -f /tmp/ghostty-refresh-out/lib/libghostty-vt.a
cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34
test -z "$(git status --porcelain third_party/ghostty | grep -v "^D ")"
echo "offline rebuild for both Darwin targets from the committed vendored tree produced libghostty-vt.a, with ZIG_LOCAL_PKG_DIR redirected outside the repo; third_party/ghostty left byte-for-byte as committed (no zig-pkg/ or .zig-cache/ written)"
'` exited 0 at 2026-11-15T06:22:25+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 043be079b8f7 with uncommitted changes:

```text
offline rebuild for both Darwin targets from the committed vendored tree produced libghostty-vt.a, with ZIG_LOCAL_PKG_DIR redirected outside the repo; third_party/ghostty left byte-for-byte as committed (no zig-pkg/ or .zig-cache/ written)
```

Commits since the review: 0.

Check FAILED: `just check` exited 1 at 2026-11-15T06:22:31+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 043be079b8f7 with uncommitted changes:

```text
ok      format check (0.1s)
error   zig-deps-seeded: failed to copy third_party/ghostty-vt-zig-deps/p/*.tar.gz into target/zig-global-cache/p/ - check the vendored tarballs are present (third_party/README.md) and the target/ directory is writable
error: recipe `zig-deps-seeded` failed with exit code 1
```

Commits since the review: 0.

Check passed: `just check` exited 0 at 2026-11-15T06:23:10+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 043be079b8f7 with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (0.5s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (0.5s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.1s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.1s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.2s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (1.2s)
ok      architecture (1.0s)
ok      recipe names (0.1s)
```

Commits since the review: 0.

Check passed: `sh -c '
set -e
ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig
rm -rf /tmp/ghostty-refresh-out /tmp/ghostty-refresh-cache /tmp/ghostty-refresh-pkg /tmp/ghostty-refresh-global-cache
mkdir -p /tmp/ghostty-refresh-global-cache/p
cp third_party/ghostty-vt-zig-deps/p/*.tar.gz /tmp/ghostty-refresh-global-cache/p/
cd third_party/ghostty
for t in aarch64-macos x86_64-macos; do
  ZIG_GLOBAL_CACHE_DIR=/tmp/ghostty-refresh-global-cache ZIG_LOCAL_PKG_DIR=/tmp/ghostty-refresh-pkg "$ZIG" build -Demit-lib-vt -Doptimize=ReleaseFast -Dsimd=true -Dtarget=$t -Dcpu=baseline -Demit-xcframework=false -Demit-themes=false --prefix /tmp/ghostty-refresh-out --cache-dir /tmp/ghostty-refresh-cache
done
cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34
test -f /tmp/ghostty-refresh-out/lib/libghostty-vt.a
test -z "$(git status --porcelain third_party | grep -v "^D " | grep -v "README.md")"
rm -rf /tmp/ghostty-refresh-out /tmp/ghostty-refresh-cache /tmp/ghostty-refresh-pkg /tmp/ghostty-refresh-global-cache
echo "offline rebuild for both Darwin targets, using the committed vendored tarballs copied into a scratch global-cache dir (never pointed at third_party/ directly, matching justfile'"'"'s zig-deps-seeded), produced libghostty-vt.a with no network and left third_party/ untouched"
'` exited 0 at 2026-11-15T06:27:29+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 043be079b8f7 with uncommitted changes:

```text
offline rebuild for both Darwin targets, using the committed vendored tarballs copied into a scratch global-cache dir (never pointed at third_party/ directly, matching justfile's zig-deps-seeded), produced libghostty-vt.a with no network and left third_party/ untouched
```

Commits since the review: 0.

Check passed: `just check` exited 0 at 2026-11-15T06:27:41+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 043be079b8f7 with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (0.4s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.1s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.1s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.1s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.2s)
ok      test (steg-app) (0.2s)
ok      tool tests (1.0s)
ok      architecture (0.7s)
ok      recipe names (0.0s)
```

Commits since the review: 0.

Check passed: `just check` exited 0 at 2026-11-15T06:28:48+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 8f81797c4b7c:

```text
ok      format check (0.1s)
ok      lint (1.2s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (2.3s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.1s)
ok      build (steg-kernel) (0.4s)
ok      test (steg-kernel) (0.8s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.4s)
ok      build (steg) (0.2s)
ok      test (steg) (0.3s)
ok      build (steg-app) (0.6s)
ok      test (steg-app) (0.7s)
ok      tool tests (1.2s)
ok      architecture (1.1s)
ok      recipe names (0.0s)
```

Commits since the review: 32, 28 without the Asdlc-Worker trailer.

Check passed: `env GHOSTTY_SOURCE_URL=http://127.0.0.1:1/unreachable just check` exited 0 at 2026-11-15T06:31:34+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 8f81797c4b7c:

```text
ok      format check (0.1s)
ok      lint (74.2s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (39.2s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.1s)
ok      build (steg-kernel) (38.3s)
ok      test (steg-kernel) (0.6s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.4s)
ok      build (steg) (0.2s)
ok      test (steg) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (1.2s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```

Commits since the review: 32, 28 without the Asdlc-Worker trailer.

Check passed: `sh -c '
set -e
ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig
rm -rf /tmp/ghostty-refresh-out /tmp/ghostty-refresh-cache /tmp/ghostty-refresh-pkg /tmp/ghostty-refresh-global-cache
mkdir -p /tmp/ghostty-refresh-global-cache/p
cp third_party/ghostty-vt-zig-deps/p/*.tar.gz /tmp/ghostty-refresh-global-cache/p/
cd third_party/ghostty
for t in aarch64-macos x86_64-macos; do
  out=$(ZIG_GLOBAL_CACHE_DIR=/tmp/ghostty-refresh-global-cache ZIG_LOCAL_PKG_DIR=/tmp/ghostty-refresh-pkg "$ZIG" build -Demit-lib-vt -Doptimize=ReleaseFast -Dsimd=true -Dtarget=$t -Dcpu=baseline -Demit-xcframework=false -Demit-themes=false --prefix /tmp/ghostty-refresh-out --cache-dir /tmp/ghostty-refresh-cache 2>&1)
  echo "$out"
  if echo "$out" | grep -qi "fetching"; then
    echo "FAIL: a fetch happened during the $t build - the vendored tarballs alone were not enough" >&2
    exit 1
  fi
done
cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34
test -f /tmp/ghostty-refresh-out/lib/libghostty-vt.a
n=$(ls /tmp/ghostty-refresh-pkg | wc -l | tr -d " ")
test "$n" = "7"
test ! -d third_party/ghostty/zig-pkg
test ! -d third_party/ghostty/.zig-cache
rm -rf /tmp/ghostty-refresh-out /tmp/ghostty-refresh-cache /tmp/ghostty-refresh-pkg /tmp/ghostty-refresh-global-cache
echo "offline rebuild for both Darwin targets from the seven committed tarballs: zero fetches (checked the build log, not just git status), exactly 7 packages resolved into the local pkg dir, libghostty-vt.a produced, and third_party/ghostty/zig-pkg and .zig-cache do not exist on disk afterward (checked directly, not through git status, which the new .gitignore entries would otherwise make blind to this)"
'` exited 0 at 2026-11-15T06:39:23+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 8f81797c4b7c:

```text


offline rebuild for both Darwin targets from the seven committed tarballs: zero fetches (checked the build log, not just git status), exactly 7 packages resolved into the local pkg dir, libghostty-vt.a produced, and third_party/ghostty/zig-pkg and .zig-cache do not exist on disk afterward (checked directly, not through git status, which the new .gitignore entries would otherwise make blind to this)
```

Commits since the review: 0.

Check passed: `just check` exited 0 at 2026-11-15T06:41:19+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 8f81797c4b7c with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (28.5s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (29.3s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.1s)
ok      build (steg-kernel) (28.2s)
ok      test (steg-kernel) (0.6s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.4s)
ok      build (steg) (0.2s)
ok      test (steg) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (1.3s)
ok      architecture (0.8s)
ok      recipe names (0.0s)
```

Commits since the review: 0.

Check passed: `just check` exited 0 at 2026-11-15T06:41:45+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.34 on task-1.34-build-with-ghostty-s-vt-engine-zig-a at 1b1c6f7ba9ae:

```text
ok      format check (0.1s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (0.4s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.1s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.1s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.1s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.2s)
ok      test (steg-app) (0.2s)
ok      tool tests (1.1s)
ok      architecture (0.7s)
ok      recipe names (0.0s)
```

Commits since the review: 5, 4 without the Asdlc-Worker trailer.

Fixed the review's findings.

[Compliance/Evidence, Important] third_party/ghostty/zig-pkg/ (687 files, 29 MB) removed.
Found the cause by experiment, not by guess: `cargo build` never writes there (build.rs's
copy_tree copies GHOSTTY_SOURCE_DIR into $OUT_DIR/src first, skipping zig-pkg, and builds
there). The writer was a direct `zig build` invocation with third_party/ghostty as cwd - used
for one of this task's own offline-rebuild proofs. Zig's build runner (lib/compiler/Maker.zig,
override_pkg_dir) resolves the directory it extracts fetched packages into, in order:
ZIG_LOCAL_PKG_DIR if set, else --system <dir>, else <build root>/zig-pkg - and that proof set
neither. Reproduced directly: a bare `zig build --help` run from third_party/ghostty populated
exactly this directory, byte-identical to the seven tarballs already vendored at
ghostty-vt-zig-deps/p/.

Fixed at the cause, not just the result: third_party/README.md now documents the mechanism and
the ZIG_LOCAL_PKG_DIR/--cache-dir-redirected, scratch-global-cache form of the direct-zig-build
proof (copying the committed tarballs into a scratch dir first, same as justfile's
zig-deps-seeded, rather than pointing ZIG_GLOBAL_CACHE_DIR at the committed directory itself),
so refreshing the pin later can't repeat this. .gitignore gets third_party/ghostty/zig-pkg/ and
.zig-cache/ as a second guard, not the fix - and because that makes `git status` blind to them,
the re-recorded offline-rebuild proof checks the directories don't exist on disk directly
(`test ! -d ...`), not through git status, and checks the build's own output never says
"fetching" rather than only counting the resolved packages. third_party/README.md also
reconciles this with its own earlier note about removing a vendored .gitignore (that one could
hide an already-tracked file; these stop one from being tracked by accident, and detection
doesn't go through git status).

State fixed to match reality: third_party/README.md now states the real committed size
(ghostty/ 23 MB, ghostty-vt-zig-deps/ 7.1 MB, ~30 MB total - not the ~54 MB this task's earlier
notes said, which double-counted the stray zig-pkg/ copy).

[Licence, Nit] third_party/README.md's Zig-deps table now names each of the seven packages'
licence and the licence file it's named in, found by extracting each tarball and reading the
file, not the project's README: uucode MIT (LICENSE.md), translate_c MIT (LICENSE), aro MIT +
Unicode-License-v3 (LICENSE, LICENSE-UNICODE), zlib's own zlib licence (LICENSE), highway
Apache-2.0 OR BSD-3-Clause (LICENSE, LICENSE-BSD3), wuffs MIT OR Apache-2.0 (LICENSE,
LICENSE-MIT, LICENSE-APACHE), pixels CC0-1.0 (LICENSE). All permissive; nothing GPL/AGPL (R59
holds) - matches what the reviewer already checked by hand.

[Bugs, Nit] justfile's zig-deps-seeded no longer swallows a failed seed. The old `cp -n ... ||
true` wasn't just hiding a failure - BSD cp (macOS) exits 1 whenever `-n` skips an
already-present destination, which is the common case here (every run after the first), so
that pattern conflated "nothing to do" with "really failed." The recipe now copies each tarball
individually, skips one already at its destination, and only fails loudly (with a message
naming what and where) when a copy actually errors or the vendored directory is empty. Verified
all three paths by hand: already-seeded (no-op, passes), first real seed (copies, passes), and a
genuine failure (vendor directory temporarily emptied - fails loudly with the intended message).

Merged origin/main twice (once resolving a real conflict in crates/kernel/src/lib.rs - kept
TASK-1.36's MovedPane export alongside this task's AC #3 test module - once a board-only
no-op). `just check` recorded clean after each. The AC #1 local proof
(GHOSTTY_SOURCE_URL=unreachable, after `cargo clean -p ghostty-vt-sys`) was re-recorded at the
new head and still passes. The AC #1 CI/DNS-block proof was also rerun fresh rather than argued
to still hold: scratch PR #26 (never merged, branch deleted), run 43686868598, blocked
tip.files.ghostty.org/deps.files.ghostty.org/codeberg.org via /etc/hosts, just check green in
5m47s at this task's real head (1b1c6f7) - stronger than inferring it from an unchanged-paths
argument, since both .cargo (unchanged) and third_party (changed this round) were named in the
prior reviewer's reasoning for why the old scratch run still covered an earlier head.

Checks recorded on this task (asdlc record-check), in order, this round:
1. The offline-rebuild proof (direct zig build against third_party/ghostty, both Darwin
   targets) with the fix - ZIG_LOCAL_PKG_DIR redirected, pointing at a scratch global-cache
   copy of the vendored tarballs rather than the committed directory - passing, before any
   edit to justfile/.gitignore.
2. `just check` after the justfile/.gitignore/README fixes and the git rm of zig-pkg/ - passing.
3. `just check` again after merging origin/main (resolved the lib.rs conflict) - passing.
4. `env GHOSTTY_SOURCE_URL=... just check` after `cargo clean -p ghostty-vt-sys` - passing
   (AC #1, local half, at the new head).
5. The offline-rebuild proof, tightened (checks the build log for "fetching", checks the
   directories don't exist on disk rather than via git status, after the .gitignore entries
   were added) - passing.
6. `just check` after the README reconciliation commit and a second (board-only) main merge -
   passing.
7. GitHub (reported here, run inside this task's own worktree never reaches it): PR 20's own
   CI runs 43685860087 (3m1s, at 8f81797) and 43686625467 (2m50s, at 1b1c6f7) green; scratch
   PR 26's run 43686868598 (5m47s, DNS-blocked, at b6cee50) green, then closed without merging,
   branch deleted.

PR: https://github.com/bruk-io/steg/pull/20, branch
task-1.34-build-with-ghostty-s-vt-engine-zig-a, pushed to 1b1c6f7, up to date with origin/main.

Follow-ups carried by @bruk.o after the clean second review at 1b1c6f7, none blocking: the justfile's doc comment above zig-deps-seeded still says cp -n is a no-op, which the recipe's own comment contradicts; decision-3's text is stale now that the spike is deleted and because vendoring the source alone does not give an offline build (seven Zig packages are vendored too), to be corrected by a decision-record pull request with nothing else in it; doc-16's open question on where vendored source lives is answered by third_party/.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-15T03:14:00Z

---

Plan approved by @bruk (e63ab21b4e98, size-3): its worker may build it.

---

author: @bruk.r2
created: 2026-11-15T04:44:00Z

---

@bruk.r2 is reviewing TASK-1.34: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r2
created: 2026-11-15T04:53:00Z

---

Review by @bruk.r2 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 1 Important, 5 nit. Cites: review. The diff does not match the plan.

[Compliance/Evidence, Important] third_party/ghostty/zig-pkg/ is committed (687 files, 29 MB, confirmed with `git ls-files`) and is byte-identical to the extracted contents of the 7 tarballs already vendored separately at third_party/ghostty-vt-zig-deps/p/ (verified by extracting each tarball and diffing; e.g. uucode-0.2.0-ZZjB... matches exactly). This directly contradicts the approved plan (file 2: "with its zig-pkg/ subdirectory deleted before committing") and third_party/README.md's own claim ("no zig-pkg/ to remove, because the published tarball doesn't carry one"). The README is right that the tarball carries none - ghostty-vt-sys's build.rs (verified against the real crate at ~/.cargo/registry/src/.../ghostty-vt-sys-0.8.0+.../build.rs) does skip zig-pkg in its copy_tree, so AC #1's cargo-build path is unaffected - but the committed tree does not match what the README describes, and nothing in the notes' 9 "Departures" declares this 29 MB. It's dead weight most likely left over from the worker's own `zig build` run inside third_party/ghostty, never cleaned up or gitignored (.gitignore only has target/). Of the disclosed "~54 MB", roughly 23 MB is the real ghostty source (matching the plan's original ~22 MB estimate), ~7.1 MB is the seven vendored Zig-package tarballs, and ~29 MB is this undeclared duplicate - state the real decomposition plainly rather than "54 MB for seven packages."

[Licence, Nit] The 7 vendored Zig packages (third_party/ghostty-vt-zig-deps/p/) are code from other projects with no licence named on the task, as .claude/rules/licence.md asks. I extracted and checked each: uucode MIT, translate_c MIT, aro MIT+Unicode-License-v3, zlib's own zlib licence, highway Apache-2.0/BSD-3, wuffs Apache-2.0/MIT, pixels CC0-1.0 - all permissive, nothing GPL/AGPL (R59 holds), but this should have been stated in third_party/README.md's table rather than left to a reviewer to check.

[Bugs, Nit] justfile's zig-deps-seeded: `cp -n ... 2>/dev/null || true` swallows a failed seed silently; a missing tarball would only surface as a confusing downstream Zig fetch failure rather than a clear message at the seeding step.

Checked and found no issue: AC #1's provenance (commit 83fb95624b0439ee660c2679b43b38c8b63769cd, sha256 ed57ae9b95581c88f2614e976c1d3f146c02d443a3c26139200eb9de6e645eb4, URL - all verified directly against the real crate's own GHOSTTY.lock on this machine, not just the README's claim of them); the local poisoned-URL proof and the scratch PR #24 / CI run 43677134619 (confirmed via `gh run view`: /etc/hosts blocks on tip.files.ghostty.org, deps.files.ghostty.org, codeberg.org only - not all network - just check passed in 4m22s); that proof's paths (.cargo, third_party, justfile) are unchanged between the recorded commit 47715cff72c0 and HEAD 043be079b8f7, so it still covers the head commit, whose own CI run (43677974737) is green in 3m9s; AC #3's test (feeds b"hello", reads back through render/row/cell API, never internals); AC #4 (spikes/task-1.1-terminal-library deleted, no remaining reference outside backlog/decision-3, which the plan correctly left stale on purpose); justfile's per-recipe Zig export (build, run, lint, test, test-all, check-packages) and check-recipes passing with zig-deps-seeded correctly marked [private] and exempt; boundaries.toml's ghostty-vt allow-list entry and kept deny-features; crates/kernel/clippy.toml untouched and still applicable (ghostty-vt does no I/O); deny.toml untouched, consistent with TASK-1.31; CI pins mise-action and runs just check on PRs, just test-all on push to main. Wall time: a clean/cold `just check` now costs ~3-4.5 minutes (head PR run 3m9s, scratch-proof run 4m22s) against recent main pushes of 9s-59s - but those main runs are mostly board-only commits against a warm cache, not a fair baseline; the real cost is the Zig link paid once per clean target/, as the notes already say, then cached to well under a second.

Size disclosed to the engineer (54 MB, seven packages not three) undersells by omission: about 29 of those MB is the undeclared zig-pkg duplicate above, not part of the seven-packages story.

---

author: @bruk.r2
created: 2026-11-15T06:51:00Z

---

@bruk.r2 is reviewing TASK-1.34: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r2
created: 2026-11-15T07:02:00Z

---

Review by @bruk.r2 (same-machine): clean. It stays In Review for a person to merge.

Findings: 0 Important, 1 nit. The diff does not match the plan.

Fresh re-review at head 1b1c6f7 (worktree build, not inherited from notes).

The prior Important finding is fixed, verified independently: third_party/ghostty/zig-pkg/ is gone (git ls-files: 0 files; filesystem: absent), and the real committed size is now stated correctly - ghostty/ 23 MB + ghostty-vt-zig-deps/ 7.1 MB = ~30 MB (confirmed with du), matching third_party/README.md.

Cause, verified against the real crate, not just the notes: ghostty-vt-sys's build.rs copy_tree copies GHOSTTY_SOURCE_DIR into $OUT_DIR/src (skip-list includes "zig-pkg", confirmed by grep against ~/.cargo/registry/src/.../ghostty-vt-sys-0.8.0+.../build.rs lines 169-195) and builds there - cargo build never writes into third_party/ghostty. The writer was a direct `zig build` invocation with third_party/ghostty as cwd, run for one of this task's own offline-rebuild proofs, with neither ZIG_LOCAL_PKG_DIR nor --cache-dir set, so Zig's default (<build root>/zig-pkg) wrote there. Fixed at the cause: every such direct-zig-build proof now redirects ZIG_LOCAL_PKG_DIR and --cache-dir to a scratch dir, copying the vendored tarballs into a scratch global-cache first (never pointing ZIG_GLOBAL_CACHE_DIR at the committed directory). The .gitignore entries (zig-pkg/, .zig-cache/) are a second guard only, and the notes and third_party/README.md say so explicitly - they would hide a reappearance from `git status` (confirmed: git status --porcelain --ignored third_party is silent even looking for ignored files), not prevent one; the real build does `just check`'s own build (ZIG_GLOBAL_CACHE_DIR -> target/zig-global-cache via .cargo/config.toml, seeded by justfile's zig-deps-seeded from the read-only vendored tarballs) plus build.rs's $OUT_DIR copy. I ran `just check` myself in a fresh detached worktree at this head: all steps passed (lint 79.6s, tests/build steg-kernel ~40-45s - the Zig link cost, cached after); `git status --porcelain` was empty afterward and third_party/ghostty/zig-pkg and .zig-cache do not exist on disk.

[Bugs, Compliance/Evidence] AC #1 offline proof at this head, independently checked: GHOSTTY_SOURCE_URL=unreachable local proof is recorded post-merge; the GitHub network-blocked half reran as a second throwaway PR (#26, branch task-1.34-scratch-network-proof-2) rather than reused from the earlier head - verified via `gh pr view 26` (closed, not merged) and `git ls-remote origin` (branch gone from origin; only a stale local tracking ref remained in my fetch) and `git merge-base --is-ancestor b6cee50 HEAD` (exit 1 - the scratch commit is not in this branch). Run 43686868598 completed success in ~5m50s. PR 20's own head run (43686625467, at 1b1c6f7) is green, 2m50s - longer than recent board-only main pushes (mostly under 2 minutes, warm cache, no Zig link paid), consistent with the notes' own account of the one-time Zig-link cost per clean runner, not a new concern.

Licences (item 4): third_party/README.md now tables all seven vendored Zig packages with licence and licence-file location, independently re-checked against what I already verified by hand in the prior review - uucode MIT, translate_c MIT, aro MIT+Unicode-License-v3, zlib's own licence, highway Apache-2.0/BSD-3, wuffs MIT/Apache-2.0, pixels CC0-1.0. Nothing GPL/AGPL; R59 holds.

Seed failure (item 5): copied justfile alone into a scratch dir with no third_party/ and ran `just zig-deps-seeded` - fails loudly with "no tarballs found under third_party/ghostty-vt-zig-deps/p/ (third_party/README.md)", exit 1, not swallowed.

check-recipes / no-mise message (item 6): `just check`'s `recipe names` step passed; grepped the real build.rs directly - the no/wrong-Zig panic reads "Install it with `mise install`, activate mise, or set ZIG to the Zig 0.17 binary." - a machine without mise gets a clear message.

boundaries/clippy/deny (item 7): boundaries.toml's [steg-kernel] keeps deny-features and gains ghostty-vt on allow, reason extended; crates/kernel/clippy.toml unchanged and still names the no-I/O disallowed-types/methods list; deny.toml has no diff against main (TASK-1.31 already covers this graph).

AC #3 (bytes-in/screen-out test): present at crates/kernel/src/lib.rs, passing in my own just check run. AC #4: spikes/task-1.1-terminal-library/ is gone; no remaining reference outside backlog/.

[Bugs, Nit] justfile:44's doc comment above zig-deps-seeded ("`cp -n` is a no-op once seeded once") contradicts the recipe body's own comment two lines below ("Copies, not `cp -n`: ... conflated 'nothing to do' with 'really failed'") - a leftover from before this round's fix to that recipe, never updated to match.

Plan-matched is recorded as "no": the notes disclose real departures from the approved plan (seven Zig packages, not three; ZIG_GLOBAL_CACHE_DIR pointing at target/, not the vendored directory; a per-recipe conditional ZIG export, not a top-level one) - each with a stated reason, so none is an Important Compliance finding, but they are departures.

No Important findings. Verdict: clean.

---
<!-- COMMENTS:END -->

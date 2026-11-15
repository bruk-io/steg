# Vendored source

Three separate things live here. The first two are for two separate network surfaces
`ghostty-vt-sys`'s build otherwise reaches at build time (doc-16 open question; R92;
decision-3) - both needed for AC #1: the workspace has to build with the network
unavailable. The third, `ghostty-vt-patched/` (TASK-1.79), is a different kind of
entry: not a network surface closed, but a small patch to the safe `ghostty-vt` crate
itself, carried here until upstream wires the same thing.

Committed size, measured with `du -sh` on this tree (not the "~54 MB" an earlier draft of this
file and the task's own notes stated - that number double-counted a stray copy, see `ghostty/`
below): `ghostty/` is 23 MB, `ghostty-vt-zig-deps/` is 7.1 MB, `ghostty-vt-patched/` is 500 KB,
about 30.5 MB added to the repository in total.

## `ghostty/`

Ghostty's own source, exactly as `ghostty-vt-sys` 0.8.0 downloads and verifies it, extracted
here instead so `cargo build` never has to fetch it:

- commit: `83fb95624b0439ee660c2679b43b38c8b63769cd`
- version: `1.4.0-main+83fb956`
- url: `https://tip.files.ghostty.org/83fb95624b0439ee660c2679b43b38c8b63769cd/libghostty-vt-source.tar.gz`
- sha256: `ed57ae9b95581c88f2614e976c1d3f146c02d443a3c26139200eb9de6e645eb4`
- licence: MIT (`ghostty/LICENSE`)

These are the same values in `ghostty-vt-sys`'s own `GHOSTTY.lock` (in the crate, under
`~/.cargo/registry/src/*/ghostty-vt-sys-0.8.0+*/GHOSTTY.lock`, or the registry's own copy of the
crate). Fetched, sha256-verified against that value, and extracted with nothing else changed - no
`zig-pkg/` to remove, because the published tarball doesn't carry one; that directory only
appears as a build-time artifact once Zig's own package manager runs against this tree (see
below), and `ghostty-vt-sys`'s `build.rs` never copies it out of a `GHOSTTY_SOURCE_DIR` anyway
(its `copy_tree` skip-list names `zig-pkg` explicitly, alongside `.git`, `.zig-cache`,
`zig-cache` and `zig-out`). A handful of git-hosting dotfiles (`.github/`, `.gitmodules`,
`.gitignore`, `.gitattributes`, `.agents/`) were removed after extraction: they're git-hosting
metadata with nothing to do with building `libghostty-vt`, and a stray `.gitignore` inside a
vendored tree can silently hide files from `git status` in the outer repository, which is exactly
the kind of surprise a vendored copy should not introduce.

`.cargo/config.toml` points `GHOSTTY_SOURCE_DIR` here, so `ghostty-vt-sys`'s `build.rs` takes its
first lookup branch (a directory with `build.zig` at `GHOSTTY_SOURCE_DIR`) and never reaches the
download path.

**A `zig-pkg/` directory can still reappear here, and must never be committed if it does.** It
is not written by `cargo build` (that copies this tree into `$OUT_DIR/src` first, skipping
`zig-pkg`, and builds there - see `build.rs`'s `copy_tree`), but by running `zig build` directly
against this directory, which is sometimes useful to verify the vendored tree offline without
going through cargo at all (e.g. the "refreshing" procedure below). Zig's own build runner
(`lib/compiler/Maker.zig`, `override_pkg_dir`) resolves the directory it extracts fetched
dependencies into, in order: `ZIG_LOCAL_PKG_DIR` if set, else `--system <dir>` if given, else
**`<build root>/zig-pkg`** - the build root being whatever directory `zig build` is run from.
Running `zig build` here with neither set (confirmed by experiment: a bare `zig build --help`
run from this directory populated exactly this) extracts every dependency it resolves into
`third_party/ghostty/zig-pkg/`, byte-identical to what's already vendored, compressed, at
`ghostty-vt-zig-deps/p/` - this is almost certainly how a 29 MB, 687-file copy of that content
ended up committed here once (found in review). `.gitignore` excludes `third_party/ghostty/zig-pkg/`
and `third_party/ghostty/.zig-cache/` as a second guard, but the real fix is at the cause: never
run `zig build` against this directory without also setting `ZIG_LOCAL_PKG_DIR` and `--cache-dir`
to somewhere outside it (the "refreshing" procedure below does this).

This doesn't reintroduce the surprise the paragraph above removed a vendored `.gitignore` to avoid:
that one could have hidden a file *git already tracked* from `git status` without anyone adding
it on purpose. These two root-level entries do the opposite - they stop an accidental `git add`
from tracking Zig's own build-time litter in the first place - and `git status`'s blind spot for
them is a known, named trade-off: checking for the directories directly (`test ! -d
third_party/ghostty/zig-pkg`), not `git status --porcelain`, is how this task's own recorded
proof confirms they're gone.

## `ghostty-vt-zig-deps/`

Separately: Zig's own package manager, resolving `ghostty/build.zig.zon`, fetches a handful of
its own dependencies over the network the first time a target is built - independent of whether
Ghostty's own source came from the network or from `GHOSTTY_SOURCE_DIR` above. Vendoring the
source tree does nothing about this; it is a second, independent network surface.

Building exactly what `ghostty-vt-sys`'s `build.rs` builds (`zig build -Demit-lib-vt
-Doptimize=ReleaseFast -Dsimd=true -Demit-xcframework=false -Demit-themes=false`, once per target
`deny.toml` checks: `aarch64-apple-darwin`, `x86_64-apple-darwin`) against an empty
`ZIG_GLOBAL_CACHE_DIR`, with network available, needs exactly these seven packages across both
targets combined - not the dozen or so `zig build --help` or a full Ghostty app build would also
fetch, which this target never uses:

| package | size | licence | licence file (inside the tarball) | why |
|---|---|---|---|---|
| `uucode-0.2.0-ZZjBPoGFVgCiflJNFc6Io8xzC67B6PA5bUkk4LT0kCxg` | 960 KB | MIT | `LICENSE.md` | direct, non-lazy dependency of ghostty's root `build.zig.zon` |
| `translate_c-2.0.0-Q_BUWltOBwBYABF84EUL208pIFeUIwbBs29FmVBaafN5` | 92 KB | MIT | `LICENSE` | fetched by `pkg/translate-c`'s own `build.zig.zon` (a local path dependency of the root) |
| `aro-0.0.0-JSD1QtuBNwCASyBtNF3pqTl_W3oAJQGEVyFAtrBSE_Pa` | 452 KB | MIT + Unicode-License-v3 | `LICENSE`, `LICENSE-UNICODE` | transitive dependency of `translate_c` above |
| `N-V-__8AAB0eQwD-…` (`zlib`) | 1.5 MB | zlib | `LICENSE` | anonymous package (no `build.zig.zon` of its own) |
| `N-V-__8AADYiAAB_…` (`pixels`) | 4 KB | CC0-1.0 | `LICENSE` | anonymous package |
| `N-V-__8AAGmZhABb…` (`highway`) | 3.4 MB | Apache-2.0 OR BSD-3-Clause | `LICENSE`, `LICENSE-BSD3` | anonymous package, SIMD - likely why `-Dsimd=true` pulls it in |
| `N-V-__8AAP5JWgCG…` (`wuffs`) | 756 KB | MIT OR Apache-2.0 | `LICENSE`, `LICENSE-MIT`, `LICENSE-APACHE` | anonymous package |

All seven are permissive; nothing GPL or AGPL (R59 holds), checked by extracting each tarball
and reading its licence file(s) directly (`.claude/rules/licence.md`'s "checked by the repository's
check command, not by eye" is about `just check-licences`'s `cargo deny` pass over the Rust
dependency graph, which these aren't part of - they're Zig packages fetched by Zig's own package
manager, outside `cargo deny`'s reach, so this table is the record for them instead).

The last four have no human-readable name in `build.zig.zon` itself (Zig calls them by their
content hash alone, prefixed `N-V-`); their names above are read from `build.zig.zon.json`, which
pairs each hash with the name and URL Zig resolved it from. Which packages a `-Demit-lib-vt`
build needs is not the same list a `--help` invocation or a full Ghostty app build would fetch
(verified: running `--help` once pulled in over a dozen more, including GUI-only ones like
`vaxis` and `gtk4-layer-shell`, that a real `-Demit-lib-vt` build for either Darwin target never
touched) - this list is deliberately only what two real, complete builds asked for, nothing a
broader probe merely explored.

Each file is named by Zig's own content hash, matching `$ZIG_GLOBAL_CACHE_DIR/p/<hash>.tar.gz`
exactly, so the filename is its own integrity check - this mirrors Zig's real global package
cache layout, just rooted inside the repo instead of under `~/.cache/zig`. Kept as the compressed
tarballs Zig itself downloads, not pre-extracted: a clean rebuild from an empty `--cache-dir`
against exactly this `p/` directory, with no other changes, produces zero `info: fetching` output
and a working `libghostty-vt`, so Zig is content to extract them itself from this form - there is
no reason to also commit the extracted trees. `.cargo/config.toml` points `ZIG_GLOBAL_CACHE_DIR`
here.

**Why not `GHOSTTY_ZIG_SYSTEM_DIR` (`zig build --system <dir>`)**, the other knob `build.rs`
exposes for exactly this: tried first, and it crashes. Zig's own `--help` says `--system
[pkgdir]` "Disable package fetching; **enable all integrations**" - on this target it turned on
system integrations (freetype, harfbuzz, fontconfig, …) that `-Demit-lib-vt` never configures,
and every worker thread panicked with `reached unreachable code` (exit 134). Pre-populating
`ZIG_GLOBAL_CACHE_DIR` has no such side effect.

## `ghostty-vt-patched/`

The safe `ghostty-vt` crate (not `ghostty-vt-sys`; a different crate, the Rust wrapper
built on top of it) does not surface OSC 133's `D` step (a command's exit code) at all
in its public API, even though `ghostty-vt-sys` carries it
(`GHOSTTY_TERMINAL_OPT_SEMANTIC_PROMPT`) - checked by reading `ghostty-vt`'s own
`terminal.rs` in full, not by its absence from a search. TASK-1.79 needs it (doc-9 R14),
so this is a small, one-file patch to that crate, carried here as a vendored `path`
dependency (workspace root `Cargo.toml`'s `[patch.crates-io]`) rather than consumed
straight from crates.io.

**Provenance**, read from the primary sources, not assumed:
- Crate: `ghostty-vt`, version `0.8.0+ghostty.1.4.0-main.83fb956`, licence Apache-2.0
  (`LICENSE`; `LICENSE-GHOSTTY`, MIT, covers only the bundled terminfo/shell-integration
  resources under `resources/`, not the crate's own Rust source).
- Repository and maintainer: `https://github.com/prabirshrestha/gpui-cn` - read directly
  from this crate's own `Cargo.toml` (`repository = "..."`) and independently confirmed
  against crates.io's owners API (`GET /api/v1/crates/ghostty-vt/owners`), which names
  the sole owner as GitHub user `prabirshrestha` (Prabir Shrestha). `ghostty-vt` is a
  sub-crate of that person's own GPUI UI-kit application, not a standalone
  terminal-engine project; worth knowing when judging how quickly an upstream
  contribution might be reviewed.
- Vendored from `~/.cargo/registry/src/*/ghostty-vt-0.8.0+ghostty.1.4.0-main.83fb956/`,
  with the registry-only files a vendored copy never needs removed (`Cargo.toml.orig`,
  `Cargo.lock`, `.cargo-ok`, `.cargo_vcs_info.json`); everything else copied unchanged.
  `crates/kernel/Cargo.toml` pins `ghostty-vt = "=0.8.0"` - not the full build-metadata
  string: semver build metadata is never part of a version *requirement* and cargo
  ignores it when matching (confirmed: writing it produces cargo's own warning,
  "includes semver metadata which will be ignored"), so `=0.8.0` is the real, and the
  most precise, constraint - it refuses any version but `0.8.0` itself, patched or not,
  which an unpatched future `0.8.1` would not satisfy. What actually selects the
  *patched* copy over crates.io's real `0.8.0` is the workspace root `Cargo.toml`'s
  `[patch.crates-io]` entry, not this requirement string on its own - that entry
  replaces every resolution of `ghostty-vt` from crates.io with the vendored copy
  outright, and the requirement only has to accept whatever version the vendored copy's
  own `Cargo.toml` declares. **What makes the patch stop applying, confirmed by
  experiment**: bumping the vendored copy's own declared version to something `=0.8.0`
  no longer accepts (tried a fake `0.8.1+test`) makes cargo print a warning naming the
  patch and saying it "was not used in the crate graph", then silently fall back to
  crates.io's real, unpatched `0.8.0` - the same would happen if the
  `[patch.crates-io]` entry were removed or misconfigured. That warning is easy to miss
  in a long build log, but the fallback is not survivable here regardless: unpatched
  `ghostty-vt` has no `Event::SemanticPrompt` variant, so `crates/kernel/src/
  terminal.rs`'s own code referencing it fails to compile outright - verified by running
  the bumped-version experiment through to a real compile error, not stopping at the
  warning.
- Checked whether the newer `ghostty-vt` 0.9.0 (`+ghostty-vt.1.0.1`, released
  2026-11-14) already wires this: it doesn't (same gap, read directly from its own
  `terminal.rs`), and it also switches `ghostty-vt-sys`'s whole vendoring model from
  "build Ghostty's own C/Zig source" (what `ghostty/` and `ghostty-vt-zig-deps/` above
  are for) to "link a prebuilt release artifact, falling back to a different, narrower
  source build" - adopting it would mean redoing this file's own `ghostty/`/
  `ghostty-vt-zig-deps/` sections for no benefit to the one gap this patch closes, so
  this stays on 0.8.0. A later upgrade is a separate, unrelated piece of work.

**The patch**, `semantic-prompt.patch` (unified diff, `src/terminal.rs` only - the
crate's one other file-level difference, `Cargo.toml`, is confirmed byte-identical to
the registry's own normalized copy): wires
`GHOSTTY_TERMINAL_OPT_SEMANTIC_PROMPT` as a new `Event::SemanticPrompt` variant,
following the crate's own existing per-callback pattern (`cb_pwd_changed`) to the
letter - one more `install!(...)` line, one more `unsafe extern "C" fn`, reading three
plain `Copy` scalars (`kind`, `has_exit_code`, `exit_code`) out of the C struct and
pushing an owned event. `command`/`error` (the C struct's other two, string-typed
fields) are left out entirely: nothing in steg reads them, and leaving them out means
this patch never has to reason about their FFI string ownership at all.

Adds no unsafe code to any of steg's own crates: `ghostty-vt` is a dependency, not a
workspace member (`Cargo.toml`'s `[workspace] members` list does not name it), so none
of this repository's own lints - `undocumented_unsafe_blocks`,
`crates/kernel/clippy.toml`'s I/O bans, `crates/kernel/src/lib.rs`'s
`#![forbid(unsafe_code)]` - apply to it, the same as they don't today for the unpatched
crate, which already does plenty of unsafe FFI work throughout this exact file. The
patch's own four lines of `unsafe` (`shared_from_userdata(ud)`, already used by every
other callback in the file; `&*event`, the same shape `cb_clipboard_write`'s `&*write`
already uses) carry a `// SAFETY:` comment following that crate's own convention.

**The changed-file notice** (Apache License 2.0, section 4: "You must cause any
modified files to carry prominent notices stating that You changed the files") is at
the top of `ghostty-vt-patched/src/terminal.rs` itself, naming what changed and
pointing back here.

**Re-verifying the patch is exactly what it claims, mechanically, not by reading it**:
```sh
# 1. A fresh pristine copy differs from the vendored one only by the
#    registry-only files (expected, never vendored) and src/terminal.rs:
diff -rq ~/.cargo/registry/src/*/ghostty-vt-0.8.0+ghostty.1.4.0-main.83fb956 \
         third_party/ghostty-vt-patched

# 2. Applying the committed patch to a fresh pristine copy reproduces the
#    vendored file byte for byte:
cp ~/.cargo/registry/src/*/ghostty-vt-0.8.0+ghostty.1.4.0-main.83fb956/src/terminal.rs /tmp/reverify.rs
patch -p0 /tmp/reverify.rs < third_party/ghostty-vt-patched/semantic-prompt.patch
diff -u /tmp/reverify.rs third_party/ghostty-vt-patched/src/terminal.rs  # empty output
```
Both ran clean while building this task.

**Dropping this patch, once upstream wires the same callback**: bump `crates/kernel/
Cargo.toml`'s `ghostty-vt` pin past whatever version does; delete this directory and
the workspace root `Cargo.toml`'s `[patch.crates-io]` entry; update
`crates/kernel/src/terminal.rs`'s `command_mark_from` to read the real
`ghostty_vt::terminal::Event`'s new variant (its exact shape is not guaranteed to match
this patch's `SemanticPromptEvent`/`SemanticPromptKind` - that function is the one
place that would need to change, not anything that calls `Terminal::take_command_marks`
downstream, since `CommandMark` is steg's own type either way).

**Proposing this upstream**: raised on TASK-1.79 as something the engineer may want to
open against `prabirshrestha/gpui-cn` - not opened by this task.

## Refreshing `ghostty/` or `ghostty-vt-zig-deps/`

A newer `ghostty-vt-sys` release pins a newer Ghostty commit in its own `GHOSTTY.lock`. To move
to it: read the new commit/version/sha256/url from that lock file, repeat the fetch-and-verify
above into `ghostty/`, and rebuild `-Demit-lib-vt` against it with an **empty**
`ZIG_GLOBAL_CACHE_DIR` and network allowed once - the error messages name exactly which packages
are missing, the same way building against this pin did. Re-derive `ghostty-vt-zig-deps/` from
whatever that run actually asks for across both Darwin targets; a newer Ghostty commit is not
guaranteed to need the same seven.

Whenever this rebuild (or any other `zig build` invocation) runs with this directory as its
build root - rather than through `cargo build`, which always builds from a copy under
`$OUT_DIR/src` - also set `ZIG_LOCAL_PKG_DIR` to somewhere outside this tree (a `/tmp` directory,
or the gitignored `target/`), and `--cache-dir` likewise. Leaving either at its default writes
into `ghostty/zig-pkg/` or `ghostty/.zig-cache/`, inside the vendored, committed tree - the
mistake the paragraph above this table exists to flag. Don't point `ZIG_GLOBAL_CACHE_DIR`
straight at the committed `ghostty-vt-zig-deps/` either, for the same reason `justfile`'s
`zig-deps-seeded` recipe doesn't: Zig writes its own compiled-artifact cache (`b/`, `h/`, `o/`,
`z/`) there too, alongside the fetched packages under `p/`. Copy the tarballs into a scratch
global-cache directory first, same as that recipe does. For example:

```sh
rm -rf /tmp/ghostty-refresh-out /tmp/ghostty-refresh-cache /tmp/ghostty-refresh-pkg /tmp/ghostty-refresh-global-cache
mkdir -p /tmp/ghostty-refresh-global-cache/p
cp third_party/ghostty-vt-zig-deps/p/*.tar.gz /tmp/ghostty-refresh-global-cache/p/
cd third_party/ghostty
for t in aarch64-macos x86_64-macos; do  # Zig's own target names (build.rs's zig_target); one -Dtarget per invocation - it isn't multi-valued
  ZIG_GLOBAL_CACHE_DIR=/tmp/ghostty-refresh-global-cache ZIG_LOCAL_PKG_DIR=/tmp/ghostty-refresh-pkg "$ZIG" \
    build -Demit-lib-vt -Doptimize=ReleaseFast -Dsimd=true -Dtarget=$t -Dcpu=baseline \
    -Demit-xcframework=false -Demit-themes=false \
    --prefix /tmp/ghostty-refresh-out --cache-dir /tmp/ghostty-refresh-cache
done
cd ../..
git status --porcelain third_party  # must print nothing
```

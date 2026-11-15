# Vendored source

Two separate things live here, for two separate network surfaces `ghostty-vt-sys`'s build
otherwise reaches at build time (doc-16 open question; R92; decision-3). Both are needed for
AC #1: the workspace has to build with the network unavailable.

Committed size, measured with `du -sh` on this tree (not the "~54 MB" an earlier draft of this
file and the task's own notes stated - that number double-counted a stray copy, see `ghostty/`
below): `ghostty/` is 23 MB, `ghostty-vt-zig-deps/` is 7.1 MB, about 30 MB added to the
repository in total.

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

## Refreshing either one

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

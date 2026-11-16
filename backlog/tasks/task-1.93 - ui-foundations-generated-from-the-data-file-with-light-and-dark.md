---
id: TASK-1.93
title: 'ui: foundations generated from the data file, with light and dark'
status: In Progress
assignee:
  - '@bruk.w7'
created_date: '2026-11-16T00:52:00Z'
updated_date: '2026-11-16T03:10:00Z'
labels:
  - size-3
milestone: m-4
dependencies:
  - TASK-1.91
  - TASK-1.92
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-3
asdlc_branch: task-1.93-ui-foundations-generated-from-the-data
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '6a1e03383d92'
asdlc_plan_approved_size: size-3
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-17 (R120, R121, R123, R124), decision-11. Depends on the package and on the data file. gpui's own colour and pixel types are used at the edge; say in the plan how a hex colour becomes one and whether anything is lost.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A tool in tools/ writes crates/ui/src/foundations/generated.rs from design/foundations.json, deterministically, with the design's names carried through; it has its own tests, and just check fails with a message saying how to regenerate when the generated file does not match the data file (R121)
- [ ] #2 foundations offers each value through the theme in force: a colour has its light and dark form and a component asks the theme for it, never one form directly; values with no pair, such as a size, are plain (R123). The form of a value with no pair is settled in the plan (doc-17's open question)
- [ ] #3 The gallery shows the foundations: every colour as a swatch with its name in light and in dark, the ramps, the type sizes and the spacing steps, and both terminal themes' 16 colours, and the pull request carries that image (R124)
- [ ] #4 A reviewer can pick any value in the design's foundations page and find it by the same name in the gallery image and in the generated code; the notes list the names and count them against the data file
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Not splitting. There is a seam (generator+foundations+theme vs. the gallery rewiring),
but AC#3 and AC#4 both need the gallery wired to real foundations, so splitting would
ship a first half with no way to check either acceptance criterion. Named for the
engineer, not acted on.

## Files touched
tools/src/steg_tools/foundations.py (new, pure parse/resolve/render) + its main() (I/O
shell); tools/tests/unit/test_foundations.py (new); tools/tests/adapter/
test_foundations_real_data.py (new); tools/pyproject.toml ([project.scripts] +
steg-generate-foundations); tools/src/steg_tools/recipes.py (ALLOWED_VERBS +
"generate"); justfile (generate-foundations, check-foundations recipes; check:'s
dependency line; capture-gallery's two capture_one calls); crates/ui/src/foundations/
mod.rs (wires in generated.rs, defines Theme/Appearance, a couple of spot-check
#[test]s); crates/ui/src/foundations/generated.rs (new, written by the tool, never
hand-edited); crates/ui/gallery/main.rs (placeholder GalleryTheme/
placeholder_colour/placeholder_text_colour removed; real foundations drawn instead);
crates/ui/CLAUDE.md (the "gallery's own chrome is exempt" paragraph is no longer true
once it draws from real foundations - rewritten).

## The generator
`tools/src/steg_tools/foundations.py`: pure functions parse foundations.json into a
small frozen-dataclass model, resolve every `{"ref": name}` (including the two-hop
`track.layout.fill.colour` -> `tint` -> `"session"` chain and `scrim.light.colour` ->
`text`, resolved to `text`'s *light* form because we're rendering the light branch),
derive one Rust identifier per design name, and render the final `.rs` text as a
string - no file I/O in any of this. `main()` is the thin shell: reads
design/foundations.json, renders, and either writes crates/ui/src/foundations/
generated.rs (no flag) or regenerates into memory and diffs byte-for-byte against the
committed file, exiting 1 with a message naming `just generate-foundations` on
mismatch (`--check`). One console script, `steg-generate-foundations`, both modes.

Two justfile recipes: `generate-foundations` runs the tool with no flag, then runs
rustfmt on that one file only (not `cargo fmt --all`, so it never reformats anything
else) - "generated then formatted", and the committed bytes are always already
rustfmt's. `check-foundations` runs the tool with `--check` (which internally
regenerates-and-rustfmts into a temp file before diffing, so the comparison is
apples-to-apples). `check-foundations` joins the `check:` recipe's dependency list,
next to `check-boundaries` (both are "does committed state match derived truth"
checks). `generate-foundations` needs a new verb in recipes.py's ALLOWED_VERBS
("generate" - added with a one-line comment, same pattern as TASK-1.91's "capture");
`check-foundations` needs none, "check" already covers it.

## Identifiers
Each design name becomes one Rust identifier: lowercase, every `.`-segment,
space-separated word, and `-`-separated word joined with `_`, non-alphanumeric
dropped. `ramp.1 violet` -> `ramp_1_violet`; `surface.ground` -> `surface_ground`;
`motion.status-fade` -> `motion_status_fade`. The generator asserts this map is
injective over the whole file and raises, naming both colliding design names, if it
isn't - tested with a deliberately-colliding fixture. Every generated item's doc
comment carries the design's own name verbatim, which is what actually satisfies
AC#4's "same name" requirement for a reviewer, not the derived identifier.

## Hex colour -> GPUI's own type, and what's lost
A bare `"#rrggbb"` string becomes a `u32` and is passed through `gpui::rgb(hex)` ->
`Rgba` (exact: `rgb()` does `u8/255.0` per channel, lossless for every 8-bit input).
Foundations then exposes `Hsla`, not `Rgba` - confirmed from the pinned 0.2.2 source
that `text_color()` takes `impl Into<Hsla>` directly and `bg()` takes `impl Into<Fill>`
with `From<Hsla> for Fill` also present, so exposing `Hsla` means no component ever
writes its own `.into()`. The `Rgba -> Hsla` step (`color.rs:589`) is where something
can be lost: hue is mathematically undefined at zero saturation, so a few of this
file's near-neutral surface colours don't round-trip bit-for-bit through hue/lightness
by hand even though the rendered colour is unaffected (hue is cosmetically irrelevant
at s=0). Neither `rgb()` nor the `Rgba->Hsla` conversion is `const fn`, so generated
colour values are private functions (`fn surface_ground_dark() -> Hsla`), not `const`
items - this costs nothing since nothing here needs a const context. `px(f32)` *is*
`const fn` (`geometry.rs:3598`), so every plain size becomes a real `pub const`.
`Duration::from_millis` is also const, so `motion.status-fade` becomes `pub const
MOTION_STATUS_FADE: std::time::Duration = Duration::from_millis(200);`.

## Theme and R123 (how a component asks, never gets one form directly)
`pub struct Theme { appearance: Appearance, session: Hsla }` - both fields private.
`pub enum Appearance { Light, Dark }`. `Theme::new(appearance, session) -> Theme` is
the only public constructor. Every value that has a light/dark pair in the data file
(colours, the three aliases, opacities, the two ramps' resolved stops and
caption_opacity, tint, scrim, status.ring's track colour, track.layout's fill,
focus_ring's colour) is a *method* on `Theme` (`theme.surface_ground()`,
`theme.blocked()`, `theme.tint()`, `theme.scrim()`, ...) - there is no public
free-standing function that returns a bare light-or-dark colour, which is what turns
R123 from a convention into a compile error. `Theme` implements `gpui::Global`
(`src/global.rs:22`, read via `cx.global::<Theme>()`/`cx.set_global()`, both pinned-
version APIs); a small `ActiveTheme` extension trait on `App`/`Context` gives
`cx.theme()` as the one spelling a component uses. "session" never appears as a
generated constant anywhere - it only enters through `Theme::new`'s `session`
parameter, and the three sites that need it (`themes[].cursor`, `focus_ring.colour`,
`tint.colour`) all read `theme.session` (private field, reached only through the
methods above) rather than a module-level value.

Colour aliases (`blocked`, `done`, `error`) are not separately-stored values: each is
a `Theme` method that returns the same resolved value as the colour it references
(`fn blocked(&self) -> Hsla { self.ramp_5_amber() }`) - no duplication, and the
design's alias name is still reachable by that exact name.

## Terminal themes (R124's "both terminal themes' 16 colours")
"steg dark" and "steg light" are flat, fixed colour sets (not toggled by the app's own
appearance) - generated as two functions, `terminal_dark(session: Hsla) ->
TerminalTheme` / `terminal_light(session: Hsla)`, each taking session the same way
`Theme::new` does, for the same reason (cursor is `"session"` in both). Inside each,
every `{"ref": ...}` (e.g. `bg_inactive: {"ref": "surface.pane"}`) resolves against
that theme's own appearance - "steg dark"'s ref resolves to `surface.pane`'s dark
value, "steg light"'s to its light value - at generation time, never at Theme's
runtime. A bare hex that happens to equal a named colour's value (`"steg
dark".bg` is the literal `"#1c1b21"`, which also happens to be `surface.raised`'s dark
value) stays a literal: the generator only recognises `{"ref": ...}` as a reference,
per design/README.md's own rule, and never infers one from a matching value.

## Plain values (doc-17's open question: the form of a value with no light/dark pair)
Proposed answer: a plain value (every entry under `type`, almost everything under
`sizes`, `motion`, `focus_ring`'s width/offset/corner_radius_addition) is a plain
`pub const`/function in `foundations::generated`, never routed through `Theme`.
Reasoning for the engineer: R123 only binds a value that *has* a light/dark form;
routing a value that doesn't through `Theme` would claim a distinction the design
never drew, which is false of the generated code the moment it compiles either way -
"what the code says must be true." Grouped the way the JSON nests them (e.g.
`nav.side`'s three fields become one small `pub const NAV_SIDE: NavSide` struct), so a
reviewer matches a group by its design name, not by hunting for scattered leaves.
Two values inside otherwise-plain groups are not plain, and are called out so nobody
misses them while implementing: `status.ring.track_colour` (a ref to `track`, which
*does* have a pair) and `track.layout.fill.colour` (resolves through `tint` to
`"session"`) - both are exposed only as `Theme` methods (`status_ring_track()`,
`track_layout_fill()`), reusing the already-generated colour rather than storing it
twice; the rest of each of those two groups' fields (diameter, thickness, inset, gap,
radius_outer/inner) stay plain consts alongside.

`radius.round` (AC#2's other open point, doc-17 says the engineer may rename it) has
no px value at all - proposed as a zero-sized marker type, `pub struct FullyRound;`,
not a number, with a doc comment carrying the JSON's own clarifying note (css's 999px
convention, "the orchestrator's choice"). This is the shape to confirm with the
engineer; the identifier itself is explicitly theirs to change per doc-17.

## The gallery (R124, AC#3)
Replaces `GalleryTheme`/`placeholder_colour`/`placeholder_text_colour`
(gallery/main.rs:16-60) with `foundations::Appearance`/`Theme`, built with a default
session colour of `ramp.1 violet`'s own resolved value (the data file's own `use` text
names it "default session colour"). Shows: every one of the 18 colours as a named
swatch in both forms, the two ramps as gradient strips with their stops, the terminal
themes' 16 ANSI colours each (two rows of 16), the type sizes (sample text at each
defined size/weight) and spacing steps (bars sized to each step), and the corners
(including a swatch for `FullyRound`). All of it drawn from `foundations::generated`
values exclusively, with no literal colour anywhere in the gallery's own drawing code
- not just because this task's job is to wire real foundations in, but because
TASK-1.94's upcoming colour-literal check will refuse exactly that once it lands, and
this task should not hand it something to immediately flag.

Risk, to check empirically early rather than at the end: the current 640x420 window
almost certainly doesn't fit all of this, and `screencapture -l<id>` only captures
what's composited - no scrolling. Plan is to size the gallery window tall enough to
show everything in one screen (a large fixed size, computed from the content, no
scroll), with sectioned captures as the fallback if that proves impractical on this
machine.

crates/ui/CLAUDE.md's "the gallery's own chrome is not a component... exempt" bullet
becomes false the moment the gallery draws from real foundations; it's rewritten to
say the gallery draws every colour, size and radius it uses from `foundations`, same
as any real component would, with no exemption left to state.

## capture-gallery's own image check (R124's "that image carries the PR")
`gallery.py`'s `shows_marker`/`decode_png_rgba`/`pixel_at` machinery is unchanged - only
the justfile's two `capture_one` call sites' literal coordinate and expected colour
move, because `(20, 50)` was the placeholder's whole-window background and a busy
gallery can't guarantee that point stays empty. New pin: a small margin at the
window's own top-left corner that layout deliberately never draws content into,
checked against `surface.ground`'s own resolved value for that run's appearance -
light `#f1efec` (241, 239, 236), dark `#0e0d10` (14, 13, 16) - at a coordinate inside
that margin (e.g. `(8, 8)`), confirmed against the real captured image once the
gallery is built, before this task is called done.

## Overlap with TASK-1.94 and TASK-1.96
Both touch `tools/` and the justfile's `check:` line. This task adds `check-foundations`
to that line and one verb ("generate") to recipes.py's ALLOWED_VERBS - flagged so
whoever lands second rebases past it rather than silently reverting it. TASK-1.94's
colour-literal check, once merged, is the reason the gallery here draws nothing but
foundations values (see above) - this task doesn't wait for TASK-1.94, it just avoids
giving it something to immediately flag. TASK-1.96 checks foundations.json against the
design's saved pages, not the generated Rust, so no code-level overlap is expected
beyond all three adding a `[project.scripts]` entry under `tools/src/steg_tools/` -
worth a glance for script-name collisions if they land close together.

## Proof
Unit (tools/tests/unit/test_foundations.py, no I/O):
- identifier derivation on representative name shapes (dotted, spaced, numbered,
  hyphenated) and the collision case (two names forced to collide; asserts the error
  names both)
- the three-way discrimination of a colour-position value: `{"ref": "x"}`, a bare
  `"#..."` literal, and the bare string `"session"` (design/README.md's own rule)
- reference resolution, including the two-hop `track.layout.fill.colour` -> `tint` ->
  `"session"` chain, and `scrim.light.colour` -> `text` resolving to `text`'s light
  form specifically (proves per-appearance direction, not just "a ref resolves")
- determinism: rendering twice from one in-memory fixture is byte-identical
- the regenerate-and-diff logic `--check` uses, against two small fixture strings
  (one matching, one not), asserting the pass/fail and the message naming `just
  generate-foundations`

Adapter (tools/tests/adapter/test_foundations_real_data.py, the one real-file
boundary): runs the generator against the real crates/ui/design/foundations.json,
asserts it parses with no error, and counts every name the data file defines
(colours + aliases + opacities + both ramps' names + every `type`/`sizes`/`motion`
key, nested or not + both theme names + focus_ring) against every identifier the
generator actually emitted, asserting equality - this is AC#4's evidence, and the two
counts go in the task's notes verbatim, not just "tests pass."

Rust (`crates/ui/src/foundations/mod.rs`): a handful of hand-picked `#[test]`s
construct `Theme::new(Appearance::Dark, ...)` and `Appearance::Light`, and assert a
few known methods return the data file's own hex, converted - proving the generated
module actually compiles and wires into `Theme` correctly. Not a re-derivation of the
whole file; that's the Python generator's job, proven above.

Staleness negative control: run once during the build, not kept as an automated test
(it would need to transiently mutate the committed data file). Edit one value in
design/foundations.json in an uncommitted scratch change, run `just check-foundations`,
confirm it fails naming `just generate-foundations`, revert, and record the exact
output in the task's notes.

Gallery capture: `just capture-gallery` run once the gallery is built; both
target/gallery/{light,dark}.png attached to the pull request (team rule for any UI
change), and the updated capture-check's new coordinate/colour confirmed against the
real images before calling this done.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From TASK-1.94's plan check (2026-11-15, @bruk.o): TASK-1.94's colour-literal check leaves crates/ui/gallery out because the gallery's colours are a placeholder today. This task makes the gallery draw everything from foundations, so whichever of the two merges second ends that exemption: the colour check then covers the gallery too, with the one known colour the capture's image check looks for taken from foundations by name.

Plan checked against doc-17 (R120, R121, R123, R124) and decision-11 by @bruk.o: ok, with two decisions that are the engineer's in approving it. (1) doc-17's open question: a value with no light and dark pair (type, sizes, the duration, the focus ring's measurements) is a plain constant and does not go through the theme; R123 binds only values that have a pair. (2) radius.round, which has no pixel value, is proposed as a marker type. A tool in tools/ writes src/foundations/generated.rs from the data file and formats it; just check regenerates and compares, failing with how to regenerate. Every paired colour is reached only through the Theme, which holds the appearance and the session colour, so a component cannot take one form directly: R123 is enforced by what is public, not by convention. The session colour is never a generated constant. The gallery draws only from foundations and shows every colour by name, the ramps, type, spacing, corners and both terminal themes; the capture's own image check moves to the window's ground colour for the appearance in force. One point for the build, not the plan: weigh a single Radius type for all four corners (three pixel sizes and fully round) against a separate marker type for the round one, so a component applies any corner the same way; say in the notes which was built and why.

Check FAILED: `bash -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 && uv run --project tools pytest -q tools/tests/unit/test_foundations_generate.py'` exited 2 at 2026-11-16T02:31:24+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 on task-1.93-ui-foundations-generated-from-the-data at 488a8b0c3355 with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmp4rzWts` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead

==================================== ERRORS ====================================
___________ ERROR collecting tests/unit/test_foundations_generate.py ___________
ImportError while importing test module '/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93/tools/tests/unit/test_foundations_generate.py'.
Hint: make sure your test modules/packages have valid Python names.
Traceback:
../../../../../.local/share/mise/installs/python/3.15.0/lib/python3.15/importlib/__init__.py:88: in import_module
    return _bootstrap._gcd_import(name[level:], package, level)
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
tools/tests/unit/test_foundations_generate.py:10: in <module>
    from steg_tools.foundations_generate import (
E   ModuleNotFoundError: No module named 'steg_tools.foundations_generate'
=========================== short test summary info ============================
ERROR tools/tests/unit/test_foundations_generate.py
!!!!!!!!!!!!!!!!!!!! Interrupted: 1 error during collection !!!!!!!!!!!!!!!!!!!!
1 error in 0.04s
```

Check passed: `bash -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 && uv run --project tools pytest -q tools/tests/unit/test_foundations_generate.py'` exited 0 at 2026-11-16T02:39:01+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 on task-1.93-ui-foundations-generated-from-the-data at 488a8b0c3355 with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpnSO5gM` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
.................................                                        [100%]
33 passed in 0.01s
```

Check passed: `bash -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 && uv run --project tools pytest -q tools/tests/adapter/test_foundations_generate_real_data.py -s'` exited 0 at 2026-11-16T02:39:21+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 on task-1.93-ui-foundations-generated-from-the-data at 488a8b0c3355 with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpO2FZgV` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
..AC#4: 54 names in crates/ui/design/foundations.json, 54 in generated.rs
.
3 passed in 0.03s
```

Check passed: `bash -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 && cargo test -p steg-ui'` exited 0 at 2026-11-16T02:41:23+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 on task-1.93-ui-foundations-generated-from-the-data at 488a8b0c3355 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.68s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/lib.rs (target/debug/deps/steg_ui-c8f1b2ea3c1c1c78)

running 5 tests
test foundations::tests::a_plain_value_with_no_light_dark_pair_needs_no_theme ... ok
test foundations::tests::a_colour_alias_matches_the_colour_it_names ... ok
test foundations::tests::appearance_toggles_to_its_own_inverse ... ok
test foundations::tests::the_session_colour_is_whatever_new_was_given ... ok
test foundations::tests::dark_and_light_pick_the_right_form_of_a_colour ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests gallery/main.rs (target/debug/deps/gallery-e19e5295ab444e60)

running 5 tests
test tests::an_unrecognised_value_falls_back_to_light_rather_than_failing ... ok
test tests::dark_is_recognised_in_any_case ... ok
test tests::light_is_explicit_too ... ok
test tests::toggling_is_its_own_inverse ... ok
test tests::unset_is_light ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_ui

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Check passed: `bash -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 && uv run --project tools pytest -q tools/tests/unit/test_foundations_generate.py tools/tests/adapter/test_foundations_generate_real_data.py'` exited 0 at 2026-11-16T02:42:45+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 on task-1.93-ui-foundations-generated-from-the-data at 488a8b0c3355 with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpCmDGVI` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
.......................................                                  [100%]
39 passed in 0.04s
```

Check passed: `bash -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 && cargo clippy -p steg-ui --all-targets -- -D warnings'` exited 0 at 2026-11-16T02:42:50+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 on task-1.93-ui-foundations-generated-from-the-data at 488a8b0c3355 with uncommitted changes:

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.50s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
```

Check FAILED: `bash -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 && just check-foundations'` exited 1 at 2026-11-16T02:43:47+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 on task-1.93-ui-foundations-generated-from-the-data at 488a8b0c3355 with uncommitted changes:

```text
FAILED  foundations (exit 1, 0.1s)
crates/ui/src/foundations/generated.rs does not match crates/ui/design/foundations.json - run `just generate-foundations` and commit the result.
full output: .quiet/foundations.log
error: recipe `check-foundations` failed on line 162 with exit code 1
```

Check passed: `bash -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 && just check-foundations'` exited 0 at 2026-11-16T02:43:58+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 on task-1.93-ui-foundations-generated-from-the-data at 488a8b0c3355 with uncommitted changes:

```text
ok      foundations (0.1s)
```

Check passed: `bash -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 && just capture-gallery && echo GALLERY_PROCESSES_AFTER_CAPTURE: $(ps aux | grep '"'"'[t]arget/debug/gallery'"'"' | wc -l)'` exited 0 at 2026-11-16T02:50:07+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 on task-1.93-ui-foundations-generated-from-the-data at 488a8b0c3355 with uncommitted changes:

```text
ok      build (gallery) (0.4s)
ok      gallery image (light) (0.4s)
ok      gallery image (dark) (0.4s)
GALLERY_PROCESSES_AFTER_CAPTURE: 0
```

Check passed: `bash -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 && cargo test -p steg-ui'` exited 0 at 2026-11-16T02:50:13+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 on task-1.93-ui-foundations-generated-from-the-data at 488a8b0c3355 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.22s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/lib.rs (target/debug/deps/steg_ui-c8f1b2ea3c1c1c78)

running 5 tests
test foundations::tests::a_colour_alias_matches_the_colour_it_names ... ok
test foundations::tests::a_plain_value_with_no_light_dark_pair_needs_no_theme ... ok
test foundations::tests::appearance_toggles_to_its_own_inverse ... ok
test foundations::tests::the_session_colour_is_whatever_new_was_given ... ok
test foundations::tests::dark_and_light_pick_the_right_form_of_a_colour ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests gallery/main.rs (target/debug/deps/gallery-e19e5295ab444e60)

running 5 tests
test tests::an_unrecognised_value_falls_back_to_light_rather_than_failing ... ok
test tests::dark_is_recognised_in_any_case ... ok
test tests::light_is_explicit_too ... ok
test tests::unset_is_light ... ok
test tests::the_gallery_theme_carries_the_appearance_asked_for ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_ui

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Check passed: `bash -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 && just check'` exited 0 at 2026-11-16T02:55:20+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 on task-1.93-ui-foundations-generated-from-the-data at 488a8b0c3355 with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (4.5s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.3s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (3.9s)
ok      test (steg-server, race-fixture) (3.9s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.4s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (1.3s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```

Check FAILED: `bash -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 && just check'` exited 101 at 2026-11-16T02:56:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 on task-1.93-ui-foundations-generated-from-the-data at 9dd6c15aee49, last 40 of 72 lines:

```text

running 2 tests
test a_version_mismatch_is_refused_naming_both_versions ... ok
test a_client_and_server_connection_shake_hands_and_exchange_a_call_in_memory ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/local_socket.rs (target/debug/deps/local_socket-1079895324ddf99e)

running 6 tests
test refuses_to_start_one_when_autostart_is_false ... ok
test spawn_fails_promptly_when_the_binarys_path_does_not_exist ... ok
test connects_without_spawning_when_a_listener_is_already_bound ... FAILED
test spawn_fails_when_the_binary_exists_but_is_not_executable ... ok
test gives_up_with_a_timeout_when_nothing_ever_binds ... ok
test starts_one_and_waits_when_nothing_is_listening_and_autostart_is_true ... FAILED

failures:

---- connects_without_spawning_when_a_listener_is_already_bound stdout ----

thread 'connects_without_spawning_when_a_listener_is_already_bound' (19939747) panicked at crates/sdk/tests/local_socket.rs:91:54:
called `Result::unwrap()` on an `Err` value: Os { code: 48, kind: AddrInUse, message: "Address already in use" }
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- starts_one_and_waits_when_nothing_is_listening_and_autostart_is_true stdout ----

thread 'starts_one_and_waits_when_nothing_is_listening_and_autostart_is_true' (19939752) panicked at crates/sdk/tests/local_socket.rs:124:5:
assertion failed: connect(&options).is_ok()


failures:
    connects_without_spawning_when_a_listener_is_already_bound
    starts_one_and_waits_when_nothing_is_listening_and_autostart_is_true

test result: FAILED. 4 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.02s

error: test failed, to rerun pass `-p steg-sdk --test local_socket`
full output: .quiet/test-steg-sdk-local-socket.log
error: recipe `check-packages` failed with exit code 101
```

Check passed: `bash -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 && just check'` exited 0 at 2026-11-16T02:57:54+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 on task-1.93-ui-foundations-generated-from-the-data at 9dd6c15aee49:

```text
ok      format check (0.2s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (4.5s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.3s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (3.9s)
ok      test (steg-server, race-fixture) (4.0s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (2.4s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (1.2s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```

Check passed: `bash -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 && uv run --project tools pytest -q tools/tests/unit/test_foundations_generate.py tools/tests/adapter/test_foundations_generate_real_data.py -s'` exited 0 at 2026-11-16T03:03:19+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 on task-1.93-ui-foundations-generated-from-the-data at 9dd6c15aee49 with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpaY6dx1` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
.........................................AC#4: 54 names in crates/ui/design/foundations.json, 54 in generated.rs
.
42 passed in 0.04s
```

Check passed: `bash -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 && just capture-gallery && echo GALLERY_PROCESSES_AFTER: $(ps aux | grep '"'"'[t]arget/debug/gallery'"'"' | wc -l)'` exited 0 at 2026-11-16T03:07:53+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 on task-1.93-ui-foundations-generated-from-the-data at 9dd6c15aee49 with uncommitted changes:

```text
ok      build (gallery) (0.4s)
ok      gallery image (light) (0.5s)
ok      gallery image (dark) (0.5s)
GALLERY_PROCESSES_AFTER: 0
```

Check passed: `bash -c 'cat /tmp/just-check-final.out'` exited 0 at 2026-11-16T03:09:29+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 on task-1.93-ui-foundations-generated-from-the-data at 9dd6c15aee49 with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.8s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (5.7s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (4.3s)
ok      test (steg-server, race-fixture) (4.0s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (2.5s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.4s)
ok      build (steg-app) (0.6s)
ok      test (steg-app) (0.8s)
ok      tool tests (1.4s)
ok      architecture (1.2s)
ok      recipe names (0.1s)
JUSTCHECK_EXIT:0
```

Check passed: `bash -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 && just check'` exited 0 at 2026-11-16T03:10:00+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.93 on task-1.93-ui-foundations-generated-from-the-data at 9dd6c15aee49 with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (4.5s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (4.0s)
ok      test (steg-server, race-fixture) (4.4s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (2.4s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (1.3s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-16T02:23:00Z

---

Plan approved by @bruk (6a1e03383d92, size-3): its worker may build it.

---

created: 2026-11-16T02:44:00Z

---

Point 2 (fully round corner): built one Radius type for all four corners, not a separate marker type. Checked gpui 0.2.2's own source first: `corner_radii` takes `Corners<AbsoluteLength>`, and `AbsoluteLength` is only `Pixels` or `Rems` - no percentage/fraction variant exists for a corner radius in this pinned version (unlike width/height, which do have `DefiniteLength::Fraction`). GPUI has no "fully round" primitive at all; the idiomatic way (and the one the data file's own comment already points at - the CSS 999px convention) is a conventionally large `Pixels` value, which paints fully round once it reaches half of whatever it rings, regardless of the exact number. So `radius.round` generates as `pub const RADIUS_ROUND: Pixels = px(999.0);` - same type as `radius.surface`/`radius.track`/`radius.inner`, applied by a component the same way as any other radius, with a doc comment explaining the convention. A marker type would have claimed a distinction GPUI itself doesn't draw.

Also recorded: TASK-1.94's own plan (see its notes) already named the same gallery-colour-literal-exemption overlap I flagged in the plan - its check's gallery exclusion is scoped to stay until this task lands.

---
<!-- COMMENTS:END -->

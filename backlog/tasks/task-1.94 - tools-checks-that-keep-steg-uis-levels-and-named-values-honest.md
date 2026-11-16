---
id: TASK-1.94
title: 'tools: checks that keep steg-ui''s levels and named values honest'
status: In Progress
assignee:
  - '@bruk.w6'
created_date: '2026-11-16T00:52:00Z'
updated_date: '2026-11-16T02:45:00Z'
labels:
  - size-2
milestone: m-4
dependencies:
  - TASK-1.91
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-4
asdlc_branch: task-1.94-tools-checks-that-keep-steg-ui-s-levels
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '43523b8d5452'
asdlc_plan_approved_size: size-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-17 (R119, R120, R126: what a program can check is a check and not a rule). The checks follow the existing tools in tools/src/steg_tools (boundaries.py is the model): every refusal carries its reason.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 just check refuses an import inside crates/ui that goes from a lower level to a higher one (foundations using atoms, atoms using molecules or organisms, molecules using organisms), with a message that says which rule and why (R119)
- [ ] #2 just check refuses a colour literal in crates/ui outside foundations, with a message naming the foundations value to use instead where one matches (R120); the plan says what counts as a literal and what the check cannot see
- [ ] #3 Each check has tests in tools/ with a case that must be refused and one that must pass, and is part of just check and of CI
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
One reviewable change: two checks, but they share one new foundations.json reader, one
pyproject dependency decision, and one justfile wiring, so splitting would duplicate the
reader across two PRs. Flagged as a judgment call, not re-decided mid-build.

## Tool: ast-grep-py, not regex, not a shelled-out CLI

Verified empirically (not from memory): `ast-grep-py` 0.51.2 publishes a
`cp315-cp315-macosx_11_0_arm64` wheel on PyPI (checked its JSON API's file list) and
installs cleanly under `uv run --python 3.15 --with ast-grep-py` with no build step,
landing in a real cp315 site-packages, not falling back to an older interpreter. CI
(`macos-26`, arm64, `uv` pinned to Python 3.15.2 via `setup-uv`) gets a pre-built wheel,
cached the same way `uv sync` already caches every other dependency - no new mise pin,
no new binary toolchain, no network fetch beyond the normal `uv sync`. Cost: ~5.5 MiB
wheel, and it is the **first runtime dependency `tools/pyproject.toml` has ever taken**
(`dependencies = []` today) - flagged to the engineer as the one call that isn't mine to
finalise alone, since `boundaries.py`/`affected.py`/`recipes.py` all get structured input
for free from a program already on the machine (`cargo metadata`, `just --dump`) and
parse nothing themselves.

Rejected: hand-written regex/tokenising over the raw source. Both checks are really
"what does this token mean in this syntactic position", and Rust's comments
(`//`, `/* nested */`), raw strings, and grouped/aliased `use {a, b as c}` lists make a
correct hand-rolled scanner a small lexer in disguise - exactly what a real tree-sitter
parse already gives for free, confirmed against: `use` declarations (plain, `super::`,
grouped `use crate::{atoms::Button, molecules::Tab}`, glob `use crate::atoms::*`, `pub use`),
`call_expression` (distinguishing the callee `rgb`/`hsla` from its arguments),
`struct_expression` (`Hsla { .. }`), `integer_literal`, `string_literal` - each read back
as a real AST node, not a text match that could land inside a comment or an unrelated
string.

## New files

- `tools/src/steg_tools/foundations.py` - shared reader for `crates/ui/design/foundations.json`.
  `read(path) -> Foundations` (a frozen dataclass); `hex_index(foundations) -> dict[str, tuple[NamedColour, ...]]`
  mapping a normalised 6-hex-digit value (lowercase, no `#`/`0x` prefix) to every
  `(name, "dark"|"light")` pair that has it - two names can share a hex in one form and
  differ in the other (`outline`/`track` coincide in dark, differ in light; noted in the
  data file's own `use` field). Resolves one level of `{"ref": ...}`, including the
  double-wrapped form on `colour_aliases[].ref` (`design/README.md`'s own documented
  shape) - aliases resolve to the same hex as what they reference, so the index itself
  doesn't need a separate alias entry; a match message below names the base colour only,
  not its alias, called out as a scoping cut, not an oversight. **TASK-1.96 should import
  this module instead of writing its own JSON reader** - said to the engineer so that
  task's plan doesn't duplicate it; if TASK-1.96 lands first, this task imports its module
  instead, whichever order the board picks.
- `tools/src/steg_tools/ui_levels.py` - the R119 check. Console script `steg-ui-levels`.
- `tools/src/steg_tools/ui_colours.py` - the R120 check, colour literals only (sizes,
  radii, durations and fonts are also named in R120 but not in this task's AC#2 - out of
  scope here, a follow-up task if wanted, not silently grown into this one). Console
  script `steg-ui-colours`.
- `tools/tests/unit/test_foundations.py`, `test_ui_levels.py`, `test_ui_colours.py`.
- `tools/pyproject.toml`: `dependencies = ["ast-grep-py>=0.51,<0.52"]`, plus the two new
  `[project.scripts]` entries.
- `justfile`: two new recipes, `check-ui-levels` and `check-ui-colours` (both already
  legal under `recipes.py`'s `ALLOWED_VERBS` - "check" is already on the list, no edit
  there), added to the `check:` recipe's dependency chain beside `check-boundaries`. No
  CI edit: `.github/workflows/ci.yml` only ever calls `just check`, so both run there for
  free once wired into it.
- Likely textual conflict with TASK-1.93 in `tools/pyproject.toml`'s `[project.scripts]`
  table and the justfile's `check:` line (it adds its own generator entrypoint and recipe)
  - a plain rebase resolves it, named here so neither worker is surprised.

## Check 1 (R119, AC#1): which import crosses levels the wrong way

**What the check sees.** Every `.rs` file under `crates/ui/src/**`, read with
`SgRoot(text, "rust")`. `crates/ui/gallery/**` is out of scope for this check entirely -
it is not one of the four levels (doc-17's own table) and CLAUDE.md already says so; this
task does not re-decide that, only encodes it as a path exclusion.

**Level of a file**: the first path segment under `src/` - `foundations`, `atoms`,
`molecules`, `organisms`, ranked 0-3 in that order. A `.rs` file directly under `src/`
that isn't one of those four (none exist today) is refused with its own message ("not
one of the four levels doc-17 names"), mirroring `boundaries.py`'s `unknown_rules` - a
silent skip would be worse than a loud refusal of a case nobody's hit yet.

**Resolving an import's target level**, from each `use_declaration` (recursing into a
`use_list` for the grouped form) and the first `scoped_identifier` of any multi-segment
path elsewhere in the file:
- `crate::<seg>::...` - target level is `<seg>` when it's one of the four names.
- `super::<seg>::...` - resolved against the *file's* own level, since every real file
  today is a level's own `mod.rs` directly under `src/<level>/` (one hop from crate
  root). **Known limitation, stated plainly, not solved here**: once a level gains its
  own submodules (TASK-1.95 or later), a `super::super::` chain's resolution depends on
  the importing file's nesting depth under its level directory, which this check does not
  attempt to derive generally - revisit when a level first grows a submodule.
- `crate::<name>` where `<name>` isn't one of the four level names directly: looked up
  against a one-time table built from `lib.rs`'s own `pub use <level>::<path>;` /
  `pub use <level>::<path> as <alias>;` lines (the crate-root re-export surface) - this
  is what closes the re-export loophole a per-file check would otherwise miss entirely:
  without it, `foundations` could reach `atoms::Button` through `use crate::Button;` if
  `lib.rs` ever re-exports it, and the per-file text would never say "atoms" anywhere.
  **What this cannot see**: a glob re-export at crate root (`pub use atoms::*;`) isn't
  expanded - expanding it needs to know every item `atoms` exports, which needs the
  compiler. Named as a real, deliberate gap; `lib.rs` has no `pub use` today at all (only
  `pub mod`), so it costs nothing to build this table now and pays off the moment the app
  needs its first real component re-exported.
- A glob `use crate::atoms::*` is itself a violation check the same as any other import
  of `atoms` - the `*` doesn't change which level is being reached into.
- Intra-level imports (`foundations::colours` importing `foundations::sizes`) are never a
  violation - R119 is about crossing levels, not organising within one.

**What this cannot see, named outright**: a colour/behaviour reached through a trait
object or a generic bound rather than a concrete path (doc-17's levels are unlikely to
need either, but if one appears, it's invisible here); a path built by a macro that
expands to a cross-level reference.

**Message** (mirrors `boundaries.py`'s `format_violation`): names the file and line, the
importing level, the level it reaches into, and R119's own text ("a level uses only the
levels before it"), e.g. `crates/ui/src/foundations/mod.rs:3: use crate::atoms::Icon;
reaches into atoms from foundations, which doc-17 R119 refuses (a level uses only the
levels before it).`

## Check 2 (R120, AC#2): colour literals

**What counts as a colour literal, concretely, grounded in the one real example in the
repo today** (`gallery/main.rs`'s `rgb(self.theme.placeholder_colour())` and the bare
`0x1c1b21` it returns) **and in what doc-17 points at as GPUI's own surface**
(`painting.rs`/`gradient.rs`, which build `Hsla`/`Rgba` values):
1. An `integer_literal` whose text matches `0x` followed by 6 or 8 hex digits (RGB or
   RGBA) - catches a bare hex constant even with no call wrapped around it (the
   `placeholder_colour` shape). A 1-4 digit hex (a bitmask, a size) is not flagged - a
   known, accepted under-reach in exchange for not flagging ordinary non-colour hex use;
   said plainly, not left implicit.
2. A `string_literal` matching `"#" + 6 or 8 hex digits"` exactly.
3. A `call_expression` whose callee's last path segment is one of a fixed name set -
   **verified against the pinned `gpui = 0.2.2`'s own public API at build time, not
   guessed now** (this session has no vendored gpui source to read; the names `rgb`,
   `rgba`, `hsla` are grounded in the one call already in this repo plus doc-17's own
   citations, and must be confirmed against `gpui` 0.2.2's actual exports before the
   pattern list is written - correcting a wrong guess here is one line, so this is cheap
   to get right at build time and risky to finalise from memory).
4. A `struct_expression` whose type name's last segment is `Hsla` or `Rgba` (qualified or
   bare) - this is the case (1)-(3) all miss: a colour built entirely from plain floats,
   with no hex and no named constructor call (e.g. `Hsla { h: 0.5, s: 0.5, l: 0.5, a: 1.0 }`).

**Where literals are allowed**: `crates/ui/src/foundations/**` only (TASK-1.93 writes
`generated.rs` there, which will be full of exactly these literal forms - this check's
exemption is that path, by construction, so 1.93 needs no exception added to this check).
`crates/ui/gallery/**` is excluded from this check too, same reasoning as check 1 - it's
the gallery's own chrome, already named exempt in CLAUDE.md and `gallery/main.rs`'s own
doc comments; this task encodes that existing decision as a path exclusion, it doesn't
make it.

**Judgment call flagged to the engineer**: a `#[cfg(test)]` module inside e.g.
`src/atoms/button.rs` is *not* exempt - a test asserting against a literal hex should name
the foundations value it expects too, same discipline as the component it's testing.
Not settled anywhere in doc-17; stated here so it's a visible choice, not a silent default.

**What this cannot see, named outright**: a hex value assembled by arithmetic
(`(0x1c << 16) | 0x1b21`, or similar) rather than written as one literal; a colour passed
as an opaque float triple with no distinguishing call or struct name. A named constant
defined outside foundations (e.g. `const BRAND: u32 = 0x1c1b21;` in `atoms/button.rs`,
used elsewhere as `button::BRAND`) is **not** a blind spot - the literal is caught at its
definition site, which is already outside foundations, so the use site needs no separate
check.

**Message, naming the foundations value where one matches** (via `foundations.hex_index`):
`crates/ui/src/atoms/button.rs:12: rgb(0x1c1b21) is a colour literal, which doc-17 R120
refuses outside foundations. This value matches surface.raised (dark) - use that named
value instead.` When the hex matches more than one name (the `outline`/`track` dark
coincidence), both are listed, sorted by name. When nothing matches: `...; no foundations
value has this colour - confirm it belongs in foundations and export it there, or that
this literal is wrong.`

## Proof (only what gets built; nothing here is cited without a test reaching it)

Pure-function unit tests, no I/O, mirroring `boundaries.py`'s style of plain dataclasses
over real files:
- `test_foundations.py`: `hex_index` - a hex with one name+form match; a hex with two
  matches (the outline/track case, built from the real coincidence in the data file); a
  hex with none; resolving the double-wrapped `colour_aliases[].ref` without raising.
- `test_ui_levels.py`: `level_of_file` for each of the four level paths, the gallery path
  (exempt/`None`), and an unplaced file (refused as unknown). One case per direction that
  must be refused: foundations→atoms, atoms→molecules, molecules→organisms (each R119's
  own ordering, not every pairwise combination - the ordering is transitive by
  construction, so these three adjacent crossings are what a negative control actually
  exercises). One case per level that must pass: an organism using a molecule, an atom and
  foundations together (the "uses everything before it" case). The crate-root re-export
  table: a name resolved through `pub use atoms::Button;`, and a glob entry
  (`pub use atoms::*;`) asserted as *not* resolvable (returns `None`, doesn't crash) -
  the deliberate gap, proven inert rather than merely described. `format_violation` names
  both levels and "R119".
- `test_ui_colours.py`: one refused case per literal form (hex int, hex string, named
  call, `Hsla`/`Rgba` struct) and its passing twin (the same file with the literal
  replaced by an unrelated non-colour value, so the test is proving the specific form is
  caught, not that any change trips it). The foundations-path and gallery-path exemptions,
  each with a would-be-violating literal that passes only because of the path. A match
  with one name, a match with two, and no match, each reflected in the message text.
  `format_violation` names "R120".
- One test per check against the real repository tree today (`workspace.repo_root()`),
  asserting zero violations - true but weak right now, since every real file is an empty
  doc-comment module; said plainly rather than oversold, and it becomes the real
  regression gate once TASK-1.93 and later tasks add real code.

Each test cited above gets a negative control at build time (per the team's testing
skill): disable the one line of logic it exercises, run it, record the failure with
`asdlc record-check`, restore, record the pass. Not run now - this is the plan, not the
build.

Not built, named as dropped rather than silently dropped: a check for the other R120
literal kinds (size, radius, duration, font) - AC#2 names colours only; pairwise testing
of every non-adjacent level crossing (foundations→organisms directly) - R119's ordering
makes the three adjacent crossings the only ones a negative control can meaningfully
distinguish from "the ordering rule is broken" generally.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan checked against doc-17 (R119, R120, R126) by @bruk.o: ok, with two things that are the engineer's in approving it. (1) It adds ast-grep-py to tools/pyproject.toml, the tools' first runtime dependency: a real parse of Rust (so comments, strings and grouped or aliased use lists are read correctly) in place of regular expressions; the worker confirmed a prebuilt wheel exists for CI's runner and Python and installs with no build step. (2) A colour literal inside a cfg(test) block is refused like any other. The level check follows re-exports through lib.rs and names what it cannot see (a glob re-export, a path built by a macro, a colour behind a trait); the colour check names what it cannot see (a value built by arithmetic, a constant defined outside foundations). Both leave crates/ui/gallery out: right for the level check, since the gallery uses every level; for the colour check it is right only while the gallery's colours are a placeholder, so the exemption is to end when TASK-1.93 makes the gallery draw from foundations (noted on TASK-1.93). Every case in the proof has a test tied to a negative control.

Check FAILED: `sh -c 'cd tools && uv run --quiet pytest -q tests/unit/test_foundations.py tests/unit/test_rust_source.py tests/unit/test_ui_levels.py tests/unit/test_ui_colours.py'` exited 2 at 2026-11-16T02:29:21+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.94 on task-1.94-tools-checks-that-keep-steg-ui-s-levels at 2606f48300ea with uncommitted changes, last 40 of 49 lines:

```text
tests/unit/test_foundations.py:5: in <module>
    from steg_tools.foundations import Foundations, NamedColour, hex_index, normalise_hex, read
E   ModuleNotFoundError: No module named 'steg_tools.foundations'. Did you mean: 'steg_tools.boundaries'?
_______________ ERROR collecting tests/unit/test_rust_source.py ________________
ImportError while importing test module '/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.94/tools/tests/unit/test_rust_source.py'.
Hint: make sure your test modules/packages have valid Python names.
Traceback:
/Users/brukhabtu/.local/share/mise/installs/python/3.15.0/lib/python3.15/importlib/__init__.py:88: in import_module
    return _bootstrap._gcd_import(name[level:], package, level)
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
tests/unit/test_rust_source.py:5: in <module>
    from steg_tools.rust_source import RustFile, rust_files
E   ModuleNotFoundError: No module named 'steg_tools.rust_source'
________________ ERROR collecting tests/unit/test_ui_levels.py _________________
ImportError while importing test module '/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.94/tools/tests/unit/test_ui_levels.py'.
Hint: make sure your test modules/packages have valid Python names.
Traceback:
/Users/brukhabtu/.local/share/mise/installs/python/3.15.0/lib/python3.15/importlib/__init__.py:88: in import_module
    return _bootstrap._gcd_import(name[level:], package, level)
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
tests/unit/test_ui_levels.py:5: in <module>
    from steg_tools.rust_source import RustFile
E   ModuleNotFoundError: No module named 'steg_tools.rust_source'
________________ ERROR collecting tests/unit/test_ui_colours.py ________________
ImportError while importing test module '/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.94/tools/tests/unit/test_ui_colours.py'.
Hint: make sure your test modules/packages have valid Python names.
Traceback:
/Users/brukhabtu/.local/share/mise/installs/python/3.15.0/lib/python3.15/importlib/__init__.py:88: in import_module
    return _bootstrap._gcd_import(name[level:], package, level)
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
tests/unit/test_ui_colours.py:3: in <module>
    from steg_tools.rust_source import RustFile
E   ModuleNotFoundError: No module named 'steg_tools.rust_source'
=========================== short test summary info ============================
ERROR tests/unit/test_foundations.py
ERROR tests/unit/test_rust_source.py
ERROR tests/unit/test_ui_levels.py
ERROR tests/unit/test_ui_colours.py
!!!!!!!!!!!!!!!!!!! Interrupted: 4 errors during collection !!!!!!!!!!!!!!!!!!!!
4 errors in 0.05s
```

Check passed: `sh -c 'cd tools && uv run --quiet pytest -q tests/unit/test_foundations.py tests/unit/test_rust_source.py tests/unit/test_ui_levels.py tests/unit/test_ui_colours.py'` exited 0 at 2026-11-16T02:29:35+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.94 on task-1.94-tools-checks-that-keep-steg-ui-s-levels at 2606f48300ea with uncommitted changes:

```text
.....................................................................    [100%]
69 passed in 0.03s
```

Check passed: `sh -c 'cd tools && uv run --quiet pytest -q tests'` exited 0 at 2026-11-16T02:30:13+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.94 on task-1.94-tools-checks-that-keep-steg-ui-s-levels at 2606f48300ea with uncommitted changes:

```text
........................................................................ [ 53%]
..............................................................           [100%]
134 passed in 1.14s
```

Check FAILED: `sh -c 'cd tools && uv run --quiet pytest -q tests/unit/test_ui_levels.py tests/adapter/test_ui_levels_real_repo.py'` exited 1 at 2026-11-16T02:30:38+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.94 on task-1.94-tools-checks-that-keep-steg-ui-s-levels at 2606f48300ea with uncommitted changes, last 40 of 81 lines:

```text
        # Non-adjacent: a comparison written as "exactly one level up" would miss this.
        file = RustFile("crates/ui/src/foundations/mod.rs", "use crate::organisms::Bar;\n")
        found = violations((file,), {})
>       assert found == (Violation(file.path, 1, "use crate::organisms::Bar;", "foundations", "organisms"),)
E       AssertionError: assert () == (Violation(pa...'organisms'),)
E         
E         Right contains one more item: Violation(path='crates/ui/src/foundations/mod.rs', line=1, use_text='use crate::organisms::Bar;', importing_level='foundations', reached_level='organisms')
E         Use -v to get more diff

tests/unit/test_ui_levels.py:183: AssertionError
_ test_violations_refuses_an_import_reached_only_through_a_crate_root_reexport _

    def test_violations_refuses_an_import_reached_only_through_a_crate_root_reexport() -> None:
        file = RustFile("crates/ui/src/foundations/mod.rs", "use crate::Button;\n")
        found = violations((file,), {"Button": "atoms"})
>       assert found == (Violation(file.path, 1, "use crate::Button;", "foundations", "atoms"),)
E       AssertionError: assert () == (Violation(pa...vel='atoms'),)
E         
E         Right contains one more item: Violation(path='crates/ui/src/foundations/mod.rs', line=1, use_text='use crate::Button;', importing_level='foundations', reached_level='atoms')
E         Use -v to get more diff

tests/unit/test_ui_levels.py:203: AssertionError
_ test_violations_counts_a_fully_qualified_expression_path_once_not_once_per_nesting _

    def test_violations_counts_a_fully_qualified_expression_path_once_not_once_per_nesting() -> None:
        file = RustFile("crates/ui/src/foundations/mod.rs", "fn f() { let _ = crate::atoms::Button::new(); }\n")
        found = violations((file,), {})
>       assert len(found) == 1
E       assert 0 == 1
E        +  where 0 = len(())

tests/unit/test_ui_levels.py:215: AssertionError
=========================== short test summary info ============================
FAILED tests/unit/test_ui_levels.py::test_violations_refuses_foundations_importing_atoms
FAILED tests/unit/test_ui_levels.py::test_violations_refuses_atoms_importing_molecules
FAILED tests/unit/test_ui_levels.py::test_violations_refuses_molecules_importing_organisms
FAILED tests/unit/test_ui_levels.py::test_violations_refuses_foundations_importing_organisms_directly
FAILED tests/unit/test_ui_levels.py::test_violations_refuses_an_import_reached_only_through_a_crate_root_reexport
FAILED tests/unit/test_ui_levels.py::test_violations_counts_a_fully_qualified_expression_path_once_not_once_per_nesting
6 failed, 33 passed in 0.05s
```

Check FAILED: `sh -c 'cd tools && uv run --quiet pytest -q tests/unit/test_ui_levels.py -k passes'` exited 1 at 2026-11-16T02:30:56+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.94 on task-1.94-tools-checks-that-keep-steg-ui-s-levels at 2606f48300ea with uncommitted changes:

```text
FF                                                                       [100%]
=================================== FAILURES ===================================
_ test_violations_passes_an_organism_using_a_molecule_an_atom_and_foundations __

    def test_violations_passes_an_organism_using_a_molecule_an_atom_and_foundations() -> None:
        file = RustFile(
            "crates/ui/src/organisms/mod.rs",
            "use crate::molecules::Tab;\nuse crate::atoms::Icon;\nuse crate::foundations::Colour;\n",
        )
>       assert violations((file,), {}) == ()
E       AssertionError: assert (Violation(pa...foundations')) == ()
E         
E         Left contains 3 more items, first extra item: Violation(path='crates/ui/src/organisms/mod.rs', line=1, use_text='use crate::molecules::Tab;', importing_level='organisms', reached_level='molecules')
E         Use -v to get more diff

tests/unit/test_ui_levels.py:191: AssertionError
________ test_violations_passes_an_organism_using_foundations_directly _________

    def test_violations_passes_an_organism_using_foundations_directly() -> None:
        # Two levels down: a comparison written as "exactly one level" would refuse this.
        file = RustFile("crates/ui/src/organisms/mod.rs", "use crate::foundations::Colour;\n")
>       assert violations((file,), {}) == ()
E       AssertionError: assert (Violation(pa...oundations'),) == ()
E         
E         Left contains one more item: Violation(path='crates/ui/src/organisms/mod.rs', line=1, use_text='use crate::foundations::Colour;', importing_level='organisms', reached_level='foundations')
E         Use -v to get more diff

tests/unit/test_ui_levels.py:197: AssertionError
=========================== short test summary info ============================
FAILED tests/unit/test_ui_levels.py::test_violations_passes_an_organism_using_a_molecule_an_atom_and_foundations
FAILED tests/unit/test_ui_levels.py::test_violations_passes_an_organism_using_foundations_directly
2 failed, 35 deselected in 0.02s
```

Check FAILED: `sh -c 'cd tools && uv run --quiet pytest -q tests/unit/test_ui_levels.py -k '"'"'crate_root_exports or reexport'"'"''` exited 1 at 2026-11-16T02:31:23+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.94 on task-1.94-tools-checks-that-keep-steg-ui-s-levels at 2606f48300ea with uncommitted changes:

```text
FF.......                                                                [100%]
=================================== FAILURES ===================================
_______________ test_crate_root_exports_resolves_a_plain_pub_use _______________

    def test_crate_root_exports_resolves_a_plain_pub_use() -> None:
>       assert crate_root_exports("pub use crate::atoms::Button;\n") == {"Button": "atoms"}
E       AssertionError: assert {} == {'Button': 'atoms'}
E         
E         Right contains 1 more item:
E         {'Button': 'atoms'}
E         Use -v to get more diff

tests/unit/test_ui_levels.py:80: AssertionError
_____________ test_crate_root_exports_resolves_an_aliased_pub_use ______________

    def test_crate_root_exports_resolves_an_aliased_pub_use() -> None:
>       assert crate_root_exports("pub use crate::atoms::Button as Btn;\n") == {"Btn": "atoms"}
E       AssertionError: assert {} == {'Btn': 'atoms'}
E         
E         Right contains 1 more item:
E         {'Btn': 'atoms'}
E         Use -v to get more diff

tests/unit/test_ui_levels.py:84: AssertionError
=========================== short test summary info ============================
FAILED tests/unit/test_ui_levels.py::test_crate_root_exports_resolves_a_plain_pub_use
FAILED tests/unit/test_ui_levels.py::test_crate_root_exports_resolves_an_aliased_pub_use
2 failed, 7 passed, 28 deselected in 0.02s
```

Check passed: `sh -c 'cd tools && uv run --quiet pytest -q tests/unit/test_ui_levels.py tests/adapter/test_ui_levels_real_repo.py'` exited 0 at 2026-11-16T02:31:46+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.94 on task-1.94-tools-checks-that-keep-steg-ui-s-levels at 2606f48300ea with uncommitted changes:

```text
........................................                                 [100%]
40 passed in 0.04s
```

Check FAILED: `sh -c 'cd tools && uv run --quiet pytest -q tests/unit/test_ui_colours.py tests/adapter/test_ui_colours_real_repo.py'` exited 1 at 2026-11-16T02:32:01+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.94 on task-1.94-tools-checks-that-keep-steg-ui-s-levels at 2606f48300ea with uncommitted changes, last 40 of 138 lines:

```text
E         
E         Right contains one more item: Violation(path='crates/ui/src/atoms/button.rs', line=1, text='0x1c1b21', matches=(('surface.raised', 'dark'),))
E         Use -v to get more diff

tests/unit/test_ui_colours.py:104: AssertionError
_____ test_violations_lists_every_matching_name_when_more_than_one_matches _____

    def test_violations_lists_every_matching_name_when_more_than_one_matches() -> None:
        file = RustFile("crates/ui/src/atoms/button.rs", "fn f() -> u32 { 0x3d3b45 }\n")
        found = violations((file,), COINCIDING_INDEX)
>       assert found[0].matches == (("outline", "dark"), ("track", "dark"))
               ^^^^^^^^
E       IndexError: tuple index out of range

tests/unit/test_ui_colours.py:110: IndexError
_ test_violations_has_no_matches_when_the_literal_matches_no_foundations_value _

    def test_violations_has_no_matches_when_the_literal_matches_no_foundations_value() -> None:
        file = RustFile("crates/ui/src/atoms/button.rs", "fn f() -> u32 { 0xabcdef }\n")
        found = violations((file,), SURFACE_RAISED_INDEX)
>       assert found == (Violation(file.path, 1, "0xabcdef", ()),)
E       AssertionError: assert () == (Violation(pa... matches=()),)
E         
E         Right contains one more item: Violation(path='crates/ui/src/atoms/button.rs', line=1, text='0xabcdef', matches=())
E         Use -v to get more diff

tests/unit/test_ui_colours.py:116: AssertionError
=========================== short test summary info ============================
FAILED tests/unit/test_ui_colours.py::test_literal_colours_catches_a_bare_hex_integer
FAILED tests/unit/test_ui_colours.py::test_literal_colours_catches_a_hex_colour_string
FAILED tests/unit/test_ui_colours.py::test_literal_colours_catches_a_named_constructor_call
FAILED tests/unit/test_ui_colours.py::test_literal_colours_catches_a_qualified_named_constructor_call
FAILED tests/unit/test_ui_colours.py::test_literal_colours_catches_an_hsla_struct_literal
FAILED tests/unit/test_ui_colours.py::test_literal_colours_catches_a_qualified_rgba_struct_literal
FAILED tests/unit/test_ui_colours.py::test_literal_colours_does_not_double_count_a_hex_argument_already_inside_a_constructor_call
FAILED tests/unit/test_ui_colours.py::test_literal_colours_still_catches_a_bare_hex_that_is_never_wrapped_in_a_call
FAILED tests/unit/test_ui_colours.py::test_violations_reports_the_same_literal_outside_either_exempt_path
FAILED tests/unit/test_ui_colours.py::test_violations_lists_every_matching_name_when_more_than_one_matches
FAILED tests/unit/test_ui_colours.py::test_violations_has_no_matches_when_the_literal_matches_no_foundations_value
11 failed, 13 passed in 0.05s
```

Check FAILED: `sh -c 'cd tools && uv run --quiet pytest -q tests/unit/test_ui_colours.py -k exempt'` exited 1 at 2026-11-16T02:32:21+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.94 on task-1.94-tools-checks-that-keep-steg-ui-s-levels at 2606f48300ea with uncommitted changes:

```text
FF..                                                                     [100%]
=================================== FAILURES ===================================
____________________ test_is_exempt_for_a_foundations_path _____________________

    def test_is_exempt_for_a_foundations_path() -> None:
>       assert is_exempt("crates/ui/src/foundations/generated.rs") is True
E       AssertionError: assert False is True
E        +  where False = is_exempt('crates/ui/src/foundations/generated.rs')

tests/unit/test_ui_colours.py:14: AssertionError
_____________________ test_is_exempt_for_the_gallery_path ______________________

    def test_is_exempt_for_the_gallery_path() -> None:
>       assert is_exempt("crates/ui/gallery/main.rs") is True
E       AssertionError: assert False is True
E        +  where False = is_exempt('crates/ui/gallery/main.rs')

tests/unit/test_ui_colours.py:18: AssertionError
=========================== short test summary info ============================
FAILED tests/unit/test_ui_colours.py::test_is_exempt_for_a_foundations_path
FAILED tests/unit/test_ui_colours.py::test_is_exempt_for_the_gallery_path - A...
2 failed, 2 passed, 19 deselected in 0.02s
```

Check passed: `sh -c 'cd tools && uv run --quiet pytest -q tests'` exited 0 at 2026-11-16T02:32:32+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.94 on task-1.94-tools-checks-that-keep-steg-ui-s-levels at 2606f48300ea with uncommitted changes:

```text
........................................................................ [ 53%]
...............................................................          [100%]
135 passed in 1.19s
```

Check FAILED: `just check-ui-levels` exited 1 at 2026-11-16T02:32:50+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.94 on task-1.94-tools-checks-that-keep-steg-ui-s-levels at 2606f48300ea with uncommitted changes:

```text
FAILED  ui levels (exit 1, 0.0s)
crates/ui/src/foundations/mod.rs:9: `use crate::atoms::Icon;` reaches into atoms from foundations, which doc-17 R119 refuses (a level uses only the levels before it: foundations < atoms < molecules < organisms).
full output: .quiet/ui-levels.log
error: recipe `check-ui-levels` failed on line 258 with exit code 1
```

Check FAILED: `just check-ui-colours` exited 1 at 2026-11-16T02:33:07+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.94 on task-1.94-tools-checks-that-keep-steg-ui-s-levels at 2606f48300ea with uncommitted changes:

```text
FAILED  ui colours (exit 1, 0.1s)
crates/ui/src/atoms/mod.rs:8: `0x1c1b21` is a colour literal, which doc-17 R120 refuses outside foundations. This value matches surface.raised (dark) - use that named value instead.
full output: .quiet/ui-colours.log
error: recipe `check-ui-colours` failed on line 262 with exit code 1
```

Check passed: `just check` exited 0 at 2026-11-16T02:38:34+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.94 on task-1.94-tools-checks-that-keep-steg-ui-s-levels at 2606f48300ea with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (4.3s)
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
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (1.4s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
ok      ui levels (0.0s)
ok      ui colours (0.0s)
```

Check passed: `just check` exited 0 at 2026-11-16T02:40:26+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.94 on task-1.94-tools-checks-that-keep-steg-ui-s-levels at 96a85fb83749:

```text
ok      format check (0.2s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (4.4s)
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
ok      test (steg-app) (0.3s)
ok      tool tests (1.1s)
ok      architecture (0.8s)
ok      recipe names (0.0s)
ok      ui levels (0.0s)
ok      ui colours (0.0s)
```

Check passed: `just check` exited 0 at 2026-11-16T02:45:53+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.94 on task-1.94-tools-checks-that-keep-steg-ui-s-levels at 5abcaef9f944:

```text
ok      format check (0.2s)
ok      lint (1.5s)
ok      boundaries (0.2s)
ok      licences (1.2s)
ok      tests (every crate) (7.4s)
ok      build (steg-sdk) (0.5s)
ok      test (steg-sdk) (0.7s)
ok      test (steg-sdk, local-socket) (0.9s)
ok      build (steg-kernel) (0.6s)
ok      test (steg-kernel) (0.9s)
ok      build (steg-server) (0.3s)
ok      test (steg-server) (5.5s)
ok      test (steg-server, race-fixture) (4.9s)
ok      build (steg) (0.1s)
ok      test (steg) (0.2s)
ok      test (steg, fixtures) (2.6s)
ok      build (steg-ui) (0.4s)
ok      test (steg-ui) (0.5s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.1s)
ok      architecture (1.0s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-16T02:18:00Z

---

Plan approved by @bruk (43523b8d5452, size-2): its worker may build it.

---
<!-- COMMENTS:END -->

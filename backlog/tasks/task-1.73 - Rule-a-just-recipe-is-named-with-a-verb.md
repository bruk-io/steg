---
id: TASK-1.73
title: 'Rule: a just recipe is named with a verb'
status: In Review
assignee:
  - '@bruk.w2'
created_date: '2026-11-15T03:12:00Z'
updated_date: '2026-11-15T03:41:00Z'
labels:
  - size-2
  - review-passed
milestone: m-0
dependencies:
  - TASK-1.33
references:
  - task-1.73-rule-a-just-recipe-is-named-with-a-verb
  - https://github.com/bruk-io/steg/pull/18
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.73-rule-a-just-recipe-is-named-with-a-verb
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'd0193248f056'
asdlc_plan_approved_size: size-2
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=0,nit=1'
asdlc_plan_matched: yes
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Asked for by Bruk on 2026-11-15: this applies to justfiles, and a just recipe needs to be a verb. A recipe is something you tell the repository to do, so its name should say the action: `just check`, `just build`, `just test`. Today several are nouns or adjectives (`boundaries`, `packages`, `affected`, and `architecture` once TASK-1.33 merges), which read as things, not commands.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A rule scoped to the justfile says a recipe's name starts with a verb in the imperative, with the reason, and gives the pattern for a variant (verb first, then what it acts on, such as test-all)
- [ ] #2 Every existing recipe that does not start with a verb is renamed, and the plan lists each old and new name for the engineer to approve before anything is renamed
- [ ] #3 Everything that names a renamed recipe is updated in the same change: CLAUDE.md, the rules, the CI workflow, the tools and their tests; the repository plugin's commands are listed for the orchestrator to change through asdlc-plugin
- [ ] #4 A check in tools/, run by just check, fails when a recipe's first word is not on a short list of verbs kept beside it, with a message that gives the rule; the list is extended by editing it, never by skipping the check
- [ ] #5 just check passes, and the old names are gone: no alias is left behind
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## 1. Rename table (every recipe, for the engineer to approve before anything is renamed)

Verb-first, "what it acts on" second, matching `test-all`/`test-tools`. Single proposal per row, no alternatives.

| Old name | New name | Reason |
|---|---|---|
| `boundaries` | `check-boundaries` | checks crate deps against `boundaries.toml`; reads as a command |
| `packages` | `check-packages` | builds+tests each package alone to prove isolation; a check, like the others in this group |
| `affected` | `show-affected` | it prints a selection, it doesn't check anything - "check" would be a wrong claim |
| `architecture` | `check-architecture` | validates `architecture/` against the LikeC4 grammar |
| `fmt-check` | `check-fmt` | already verb-second; this just reorders it to the pattern |
| `default` | *unchanged* | just's own reserved entry point for a bare `just` (`[private]`), never typed by name - not a command a person gives, exempt rather than renamed |
| `check` | *unchanged* | already a verb |
| `test`, `test-all`, `test-tools` | *unchanged* | already verb-first |
| `fmt` | *unchanged* | already a verb |
| `lint` | *unchanged* | already a verb |
| `build` | *unchanged* | already a verb |
| `run` | *unchanged* | already a verb |
| `setup` | *unchanged* | already a verb |
| `deny` | *unchanged* | already reads as a command ("deny a disallowed licence"); **flagging for the engineer**: it's not one of the five named in the brief, so the default here is to leave it, but leaving it means `deny` goes on the check's allowed-verb list as a verb in its own right, next to `check`/`test`/`build`. Say if that's wrong. |
| *(new)* | `check-recipes` | the mechanical check itself (section 3); joins `just check` |

Empirically confirmed on just 1.59.0: `check` and `check-fmt` coexist with no ambiguity (`just` matches recipe names exactly, not by prefix) - tested against a throwaway two-recipe justfile.

Resulting allowed-verb list (9 verbs, covering all 16 recipes after the rename): `check`, `test`, `build`, `fmt`, `lint`, `run`, `setup`, `deny`, `show`.

## 2. Every place a renamed recipe is named

**Must edit, in this task:**
- `justfile` - the five `recipe-name:` lines themselves, the `check:` dependency list (`fmt-check lint boundaries deny test test-tools architecture` → `check-fmt lint check-boundaries deny test test-tools check-architecture check-recipes`), and the new `check-recipes:` recipe.
- `CLAUDE.md:11` - "`just check` ... format, lint, licences, the affected tests, and the tools' tests" is already stale (missing boundaries/architecture) and must be rewritten regardless; do it now in the same pass naming the real steps, not the old recipe tokens.
- `.claude/rules/architecture.md:19` - "`just boundaries` fails on anything added that isn't listed" → `just check-boundaries`.
- `boundaries.toml:3` - comment "Checked by ... run as `just boundaries`" → `just check-boundaries`.
- `crates/kernel/Cargo.toml:11` - comment "`just boundaries` checks that this" → `just check-boundaries`.
- `tools/src/steg_tools/boundaries.py:6` - docstring "`just boundaries` fails on any dependency ..." → `just check-boundaries`.
- `tools/pyproject.toml` - new `[project.scripts]` entry `steg-recipes = "steg_tools.recipes:main"` (section 3; easy to miss, and without it `check-recipes` has nothing to call).

**Searched, no renamed recipe named - no edit needed:**
- `.github/workflows/ci.yml` - names only `check` and `test-all`, neither renamed.
- `devs/`, `architecture/`, `asdlc.toml`, `devs/bruk.yaml`, `devs/example.yaml` - no `just` recipe references at all.
- `tools/src/steg_tools/affected.py:10`, `workspace.py:3` - name `just test` and the tool binaries `steg-affected`/`steg-boundaries` (python script names, not just-recipe names); unaffected.
- `tools/tests/unit/test_affected.py:52` - `"packages"` there is a `cargo metadata` JSON key in a test fixture, not a recipe reference.
- `spikes/*/README.md` - name `just test`/`just check` only, neither renamed.
- `.claude/rules/testing-rust.md`, `.claude/rules/licence.md`, `.claude/rules/rust.md` - no renamed recipe named.

**Orchestrator-only, listed not edited** (repository plugin, changed through asdlc-plugin after merge):
- `plugins/bruk-io-steg/commands.toml:10,15,20,21,24,26,27` - `just fmt-check`, `just boundaries` (and `needs` lines naming `just setup`, unaffected).
- `plugins/bruk-io-steg/rules/kernel.md:10` - `just boundaries`.
- `plugins/bruk-io-steg/rules/sdk.md:10` - `just boundaries`.
- `plugins/bruk-io-steg/rules/testing-rust.md:9-11` - `just affected`, `just test-all`, `just check`.

**Out of scope, informational only** (never hand-edited per CLAUDE.md): several `backlog/` task/decision docs name the old recipes in their own historical text (e.g. `task-1.33`, `task-1.5`, `task-1.71`'s AC#2 names `just boundaries` literally). Not touched by this task.

## 3. The check (`check-recipes`)

- New module `tools/src/steg_tools/recipes.py`, same split as `boundaries.py`: a pure core, a thin `main()`.
  - `ALLOWED_VERBS: frozenset[str]` as an in-module constant, next to a comment saying it's extended by editing, never by skipping the check (mirrors `GLOBAL_FILES` in `affected.py`). An in-module constant, not a root `verbs.toml`, because `tools/` is already in `affected.py`'s `GLOBAL_FOLDERS` - a root TOML file would need a `GLOBAL_FILES` edit too (like `boundaries.toml`/`deny.toml` have) or editing it would silently select no tests; this way it needs nothing extra.
  - `Recipe` frozen dataclass: `name: str`, `private: bool`.
  - `recipes_from_dump(dump: dict) -> tuple[Recipe, ...]` - pure, parses the shape `just --dump --dump-format json` produces (confirmed on the pinned 1.59.0: top-level `"recipes"` keyed by name, each with a `"private"` bool).
  - `first_word(name: str) -> str` - splits on `-`, takes the first segment.
  - `offenders(recipes, verbs) -> tuple[Recipe, ...]` - pure: every *public* recipe (`private is False`) whose `first_word` isn't in `verbs`. A private recipe is exempt because it's read from `"private"` in the dump, not matched by the literal name `"default"` - the exemption is "nobody types this by name," not "this one recipe is special."
  - `format_offender(recipe) -> str` - message names the recipe, the offending word, the file to extend, and the reason (mirrors `format_violation`): "`just <name>` isn't named with a verb: '<word>' isn't on the allowed list in tools/src/steg_tools/recipes.py. A recipe is something you tell the repository to do, so its name starts with the action, such as `test-all` or `check-boundaries`. Extend the list there if this is genuinely a new verb; never skip this check."
  - `main()`: `workspace.repo_root()` → `workspace.just_dump(root)` (new function, section below) → `recipes_from_dump` → `offenders` → print each, return 1 if any else 0. Same not-a-git-repo / command-failed error handling as `boundaries.main()`.
- `workspace.py` gets one new function, `just_dump(root: Path) -> dict[str, object] | None`, calling `just --justfile <root>/justfile --dump --dump-format json`, mirroring `cargo_metadata`'s shape and error handling (`None` on nonzero exit). Shared here rather than duplicated, same reasoning the module's docstring already gives for `cargo_metadata`.
- `tools/pyproject.toml`: add `steg-recipes = "steg_tools.recipes:main"` under `[project.scripts]`.
- `justfile`: new recipe, grouped with the other mechanical checks, no `rust ==` gate (it doesn't need the Rust workspace, only the justfile itself):
  ```
  # a recipe's name starts with a verb on the list kept in tools/src/steg_tools/recipes.py (.claude/rules/just-recipes.md)
  check-recipes:
      @{{quiet}} "recipe names" -- uv run --quiet --project tools steg-recipes
  ```
  Added to `check:`'s dependency list (section 2).
- Tests, both tiers the repo already uses for this kind of tool:
  - `tools/tests/unit/test_recipes.py` - pure, on plain dicts/dataclasses: `first_word`, `offenders` (a noun-first name is an offender, a verb-first one isn't, a private recipe is never an offender regardless of its name, more than one offender each reported), `format_offender` names the recipe/word/file, `recipes_from_dump` parses a hand-built dump-shaped dict.
  - `tools/tests/adapter/test_recipes_real_workspace.py` - mirrors `test_affected_real_workspace.py`: runs `workspace.just_dump` against this repository's real justfile and asserts `offenders(...) == ()` - the mechanical proof that the real file, after the rename, has none left. Also asserts the dump's top-level `"aliases" == {}`, as a second mechanical proof that AC#5's "no alias is left behind" holds, not just an assertion in prose.

## 4. The rule

New file `.claude/rules/just-recipes.md`, path-scoped to the justfile only:

```
---
paths:
  - "justfile"
---
# A recipe's name is a verb

Applies to steg's justfile.

- A recipe is something you tell the repository to do, so its name is the command: `just build`, `just test`, not `just packages`. Start it with a verb in the imperative.
- A variant names the verb first and what it acts on after: `test-all`, `test-tools`, `check-boundaries`.
- `check-recipes` (tools/src/steg_tools/recipes.py) is the mechanical half of this: it fails on any public recipe whose first word isn't on its list, with the reason in its message. Extend that list there when a recipe is genuinely a new action; never skip the check to make a name fit.
- A private recipe, such as just's own `default` (the bare `just`'s entry point, never typed by name), is exempt: the rule is about the word a person types, and nobody types that one.
```

Only the judgement half lives here (the variant pattern, why the exemption holds, where the mechanical half lives); the allowed-verb list and the fail condition are the check's job, not restated as bullets. Will run the authoring-kit linter after writing it and fix whatever it flags.

## 5. Coexistence with TASK-1.34 and TASK-1.71

- **TASK-1.34** (Zig variables, a `setup` step) - low risk: it touches variables and extends `setup`, which is already on the allowed-verb list and isn't renamed. The only shared surface is the `justfile` file itself (ordinary merge conflict, not a logic conflict). Once `check-recipes` lands, any recipe TASK-1.34 adds is mechanically held to the same convention from then on.
- **TASK-1.71** (boundaries/CLAUDE.md-per-package/lints) collides in two named places:
  - `justfile`'s `check:` dependency line: TASK-1.71's brief says it "adds `packages` to `check`"; this task renames `packages` → `check-packages` and separately adds `check-recipes` to that same line. Whichever lands second has a textual merge conflict on one line, not a design conflict - resolve by keeping both additions, using the new name.
  - `.claude/rules/architecture.md:19`: TASK-1.71's AC#5 removes lint-covered lines from that file and replaces them with the lint's name; line 19 is exactly the line this task edits (`just boundaries` → `just check-boundaries`). If TASK-1.73 lands first, TASK-1.71 finds the line already renamed and does its own edit (remove/replace) on top of that - no extra work. If TASK-1.71 lands first, this task's edit to that line may already be gone (replaced with a lint name); then this task's step for that file becomes "confirm the line is gone, nothing to rename" instead of an edit - a smaller task, not a different one.
  - Recommendation: no strict ordering is required, but landing TASK-1.73 first avoids TASK-1.71 having to touch a line this task is also touching. Either order, this task rebases onto whatever `justfile`/`architecture.md` state exists at merge time and reapplies the same renames mechanically - no design change either way.

## Proof steps (each recorded with `asdlc record-check TASK-1.73 --in <worktree> --as bruk.w2 -- <command>`, long timeouts)

1. Baseline before any edit: `just check` (establishes the starting point).
2. After the rename and the new check/rule/tests are in place: `just check` passes.
3. Demonstration that `check-recipes` actually catches a bad name - recorded even though reverted: temporarily add a noun-named recipe (e.g. `nope:` with a trivial body) to the justfile, run `just check-recipes`, confirm it fails with the message naming the rule and the file to extend, then `git -C <worktree> checkout -- justfile` to revert before continuing.
4. `uv run --project tools pytest -q tools/tests` (or via `just test-tools`) for the new unit and adapter tests specifically, then the full `just check` again to confirm the suite as a whole.
5. A grep across the repository (outside `backlog/` and `plugins/bruk-io-steg/`) for the five old names used as `just <old-name>`, confirming none remain: `sh -c 'git -C <worktree> grep -n -E "just (boundaries|packages|affected|architecture|fmt-check)\\b" -- . ":!backlog" ":!plugins/bruk-io-steg"'` - expected to find nothing (exit 1 from grep is the pass).

Commands go through `just` except step 3 (a deliberate, reverted demonstration of the justfile itself) and step 5 (a verification grep, not a build step).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan checked by @bruk.o against the engineer's words (a just recipe is a verb): ok. Five renames, a check that reads the recipes from just itself and fails on a first word not on a list of verbs, a path-scoped rule, and every reference updated in the same change, with no alias left. It collides on two lines with TASK-1.71 (the check recipe's dependency list, and one line of architecture.md); whichever lands second rebases. The plugin's commands and rules are the orchestrator's, through asdlc-plugin, after the merge.

Check passed: `just check` exited 0 at 2026-11-15T03:31:59+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.73 on task-1.73-rule-a-just-recipe-is-named-with-a-verb at 18cead8d39a5:

```text
ok      format check (0.1s)
ok      lint (14.4s)
ok      boundaries (0.2s)
ok      licences (0.6s)
skip    tests: this change touches no crate
ok      tool tests (0.5s)
ok      architecture (0.8s)
```

Check FAILED: `just check-recipes` exited 1 at 2026-11-15T03:34:01+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.73 on task-1.73-rule-a-just-recipe-is-named-with-a-verb at 18cead8d39a5 with uncommitted changes:

```text
FAILED  recipe names (exit 1, 0.0s)
`just nope` isn't named with a verb: 'nope' isn't on the allowed list in tools/src/steg_tools/recipes.py. A recipe is something you tell the repository to do, so its name starts with the action, such as `test-all` or `check-boundaries`. Extend the list there if this is genuinely a new verb; never skip this check.
full output: .quiet/recipe-names.log
error: recipe `check-recipes` failed on line 103 with exit code 1
```

Check passed: `just check` exited 0 at 2026-11-15T03:34:35+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.73 on task-1.73-rule-a-just-recipe-is-named-with-a-verb at 18cead8d39a5 with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (19.3s)
ok      tool tests (0.6s)
ok      architecture (2.0s)
ok      recipe names (0.1s)
```

Check passed: `sh -c 'git grep -n -E "just (boundaries|packages|affected|architecture|fmt-check)\b" -- . ":!backlog" ":!plugins/bruk-io-steg"; test $? -eq 1'` exited 0 at 2026-11-15T03:34:45+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.73 on task-1.73-rule-a-just-recipe-is-named-with-a-verb at 18cead8d39a5 with uncommitted changes:

```text

```

Check passed: `just check` exited 0 at 2026-11-15T03:35:38+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.73 on task-1.73-rule-a-just-recipe-is-named-with-a-verb at 56eada5b62ba:

```text
ok      format check (0.1s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (0.4s)
ok      tool tests (0.2s)
ok      architecture (1.0s)
ok      recipe names (0.1s)
```

Built as planned, no departures.

AC#1 (a rule scoped to the justfile, with the reason, pattern and variant): .claude/rules/just-recipes.md, paths: ["justfile"]. States the reason (a recipe is a command), the test-all/check-boundaries variant pattern, the default exemption, and points to check-recipes for the mechanical half.

AC#2 (every renamed recipe, plan lists old/new names for approval first): engineer approved the plan's rename table (boundaries->check-boundaries, packages->check-packages, affected->show-affected, architecture->check-architecture, fmt-check->check-fmt; default and deny left as-is) before any edit, recorded on the task.

AC#3 (everything that names a renamed recipe updated in the same change): CLAUDE.md:11, .claude/rules/architecture.md:19, boundaries.toml:3, crates/kernel/Cargo.toml:11, tools/src/steg_tools/boundaries.py:6 all updated in commit 56eada5. Confirmed by the final grep proof step (below) that no old name survives outside backlog/ and the plugin.

Repository plugin lines left for you to change through asdlc-plugin (confirmed still current on origin/main as of this rebase):
- plugins/bruk-io-steg/commands.toml, [lint]: `run = "just fmt-check && just lint && just boundaries && just deny"` -> needs fmt-check->check-fmt, boundaries->check-boundaries
- plugins/bruk-io-steg/rules/kernel.md:10 - "`just boundaries` fails on anything added"
- plugins/bruk-io-steg/rules/sdk.md:10 - "`just boundaries` fails on anything added"
- plugins/bruk-io-steg/rules/testing-rust.md:9-11 - "`just affected` prints what it chose" (affected->show-affected); test/test-all/check unaffected

AC#4 (a check in tools/, run by just check, fails on a bad first word, with a message naming the rule; list extended by editing, never skipping): tools/src/steg_tools/recipes.py, ALLOWED_VERBS frozenset with a comment saying so; wired into `check:`'s dependency list in the justfile as `check-recipes`. Proved by the deliberate, reverted demonstration: added a noun-named `nope:` recipe, `asdlc record-check -- just check-recipes` recorded exit 1 with the message naming the file to extend, then reverted and recorded `just check` passing again.

AC#5 (just check passes, old names gone, no alias left): `asdlc record-check -- just check` recorded exit 0 both right after the rename and again after rebasing onto origin/main (which had picked up TASK-1.71's review and TASK-1.34's/TASK-1.72's unrelated checks meanwhile - no new recipe appeared on main to hold to the rule). tools/tests/adapter/test_recipes_real_workspace.py asserts the real justfile has zero offenders and `aliases == {}` (mechanical proof nothing was left as an alias), and passed in the recorded `just check`/`just test-tools` runs. Final grep across the repository, excluding backlog/ and plugins/bruk-io-steg/, for `just (boundaries|packages|affected|architecture|fmt-check)` found nothing - recorded (sh -c, exit 0 meaning zero matches).

Coexistence: rebased cleanly onto origin/main just before pushing; the only non-backlog difference was plugins/bruk-io-steg/commands.toml and plugins/team/* (orchestrator and review-policy updates), no justfile changes landed from TASK-1.34/TASK-1.71/TASK-1.72 yet. If either lands a recipe after this merges, check-recipes will hold it to the rule automatically.

All proof steps recorded with asdlc record-check: baseline just check (exit 0), the reverted nope: demonstration (just check-recipes, exit 1), just check after reverting (exit 0), just check again after the rebase (exit 0), and the final grep (exit 0).
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-15T03:31:00Z

---

Plan approved by @bruk (d0193248f056, size-2): its worker may build it.

---

author: @bruk.r2
created: 2026-11-15T03:36:00Z

---

@bruk.r2 is reviewing TASK-1.73: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r2
created: 2026-11-15T03:41:00Z

---

Review by @bruk.r2 (same-machine): clean. It stays In Review for a person to merge.

Findings: 0 Important, 1 nit. The diff matches the plan.

[Bugs, Nit] justfile:49 - inside check-packages, the skip message still reads "skip    packages: no Rust workspace yet". The recipe itself was correctly renamed to check-packages everywhere else (the AC#3/AC#5 grep for `just packages` rightly finds nothing), but this one line prints a bare "packages" in the pattern `skip <recipe-name>:` used by every other recipe's skip message (compare check-boundaries's "skip boundaries"). Not an AC#5 violation since the grep that AC checked for was `just <old-name>`, but it's the one surviving spot a reader would notice the old noun in output.

Everything else checked out:
- No leftover `just boundaries|packages|affected|architecture|fmt-check` reference anywhere outside backlog/ and plugins/bruk-io-steg/ (re-ran the task's own grep plus a broader scan myself).
- check-recipes genuinely reads `just --dump --dump-format json` (workspace.just_dump), and the Check FAILED entry for the deliberate `nope:` recipe is a real recorded run (exit 1, message naming the file), not just described in prose.
- Private-recipe exemption is by the dump's `"private"` bool, not by name: `default` isn't on ALLOWED_VERBS, yet the real-workspace adapter test and my own `just check-recipes` run both pass, which only works if the private flag is what's exempting it.
- No alias left behind: no `alias` line in the justfile, and the adapter test's `dump["aliases"] == {}` passed.
- CI workflow (.github/workflows/ci.yml) names only `check`/`test-all`, neither renamed - nothing to update there, as the plan said.
- Independently reran `just check-recipes` (ok) and the tools test suite (40 passed) on this head commit myself, not just trusting the recorded notes.
- Checked PR 18's CI run directly: run 43673097145, conclusion SUCCESS, on headSha 56eada5b62ba32728058d8bb689e0e0b1c4e1a58, which matches the PR's current head and the task's recorded "Check passed" commit.
- Coexistence with TASK-1.71 (PR 17): the two shared lines (justfile's check: dependency list, and .claude/rules/architecture.md's last bullet) are both edited by each PR, so whichever lands second gets an ordinary textual merge conflict - nothing is silently undone there. TASK-1.71 does add new files (crates/*/CLAUDE.md, .claude/rules/kernel.md, .claude/rules/sdk.md) that reference the old `just boundaries`/`just packages` names; those are new, non-conflicting files, so if TASK-1.71 merges after TASK-1.73 they will reintroduce old names outside backlog/plugins with no merge conflict to flag it, and check-recipes can't catch it since it checks recipe names, not prose references to them. That's a risk for TASK-1.71's own review when it lands, not a defect in this diff.

---
<!-- COMMENTS:END -->

---
id: TASK-1.38
title: 'Kernel: resolve a session, tab or pane from a path'
status: In Progress
assignee:
  - '@bruk.w3'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T03:59:00Z'
labels:
  - size-2
milestone: m-0
dependencies:
  - TASK-1.35
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-8
asdlc_branch: task-1.38-kernel-resolve-a-session-tab-or-pane
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '218939872684'
asdlc_plan_approved_size: size-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-15 R10 and doc-2 section 4. This is the rule a script relies on to act on the right pane.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A part is tried as a full identifier, then a label without regard to case, then a position, then an identifier suffix of four or more characters, stopping at the first kind that matches
- [ ] #2 A target that matches nothing, or more than one thing, is an error that lists the candidates and does nothing
- [ ] #3 A label containing `/`, or made only of digits, is refused
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Scope and the sdk/kernel split (doc-16 R105)

Nothing new goes in the sdk. Argued, not assumed:

- A target's written form crosses the API as a plain `String` (doc-2 §2: a method's
  target parameter is the positional, mechanically). A string is already a shape every
  consumer can decode; there is no new type to define once for R105's sake.
- The four match kinds and their order (R10) are behaviour, not a shape. R105 pulls in
  identifiers, statuses and layouts because those are decoded off the wire; "try id, then
  label, then position, then suffix" is never decoded off anything — it is a rule the
  kernel applies to data it already owns. It stays in the kernel.
- The error a client sees when a target does not resolve is **not** defined here either.
  `kernel::Error` already carries rich, kernel-only variants that need a future mapping to
  a wire shape (`SessionNotFound(SessionId)`, `PositionOutOfRange{len,position}`) — that
  mapping is TASK-1.43's job (the API description), not this task's. The two new variants
  below (`TargetNotFound`, `TargetAmbiguous`) follow that same precedent. Their
  `Candidate.id` field is the id's `Display` string (hex, §6's `--json` contract: "full
  identifiers"), so nothing is lost by carrying a `String` instead of a typed id.
- Context substitution (doc-2 §4: a left-out part "comes from the context") is **not**
  this task's job. The context variables (`STEG_SESSION`, `STEG_TAB`, `STEG_PANE`) "carry
  identifiers, never labels" — so filling them in is a lookup the CLI already can do
  itself, with no matching and no ambiguity. By the time a path reaches the kernel, every
  part a caller wants resolved by matching is present; a caller (CLI today, the server
  later) supplies a default id for any level it chooses not to spell out. This also means
  this task has no dependency on TASK-1.37 (current tab/current pane): the design takes a
  caller-supplied default id instead of reading any "current" field, so it slots into
  whatever TASK-1.37 adds later without changing shape.

Blast radius: only `crates/kernel` and `crates/sdk` exist in this repository today (no
server, CLI or app crate yet), so every signature change below is contained to the kernel
crate's own call sites and tests.

## Files

- New `crates/kernel/src/target.rs`: the generic match-one-part algorithm, the
  `Candidate` type, and the per-level `resolve_*` methods added to `Tree`, `Session` and
  `Tab`.
- New `crates/kernel/src/label.rs`: the one place a label is validated.
- `crates/kernel/src/error.rs`: four new `Error` variants; drop `Copy` from the derive
  (every new variant owns a `String`/`Vec`, and no existing use of `Error` relies on
  `Copy` — every site is `?`, `.ok_or`, `.expect` or `assert_eq!`).
- `crates/kernel/src/pane.rs`, `tab.rs`, `session.rs`: `set_label` returns
  `Result<(), Error>` and calls `label::validate`.
- `crates/kernel/src/tree.rs`: `new_session` returns `Result<SessionId, Error>` (label
  validated before any id is minted, matching the existing "never mint on a failed call"
  comment); `new_tab` propagates `set_label`'s new `Result`.
- `crates/kernel/src/lib.rs`: export `target::Candidate` alongside `Error`.
- Existing tests in `tree.rs`, `session.rs`, `tab.rs`, `pane.rs` that call `set_label` or
  `new_session` get an `.unwrap()`/`?` added; no behaviour in them changes.

## Design

### `label::validate`

```rust
pub(crate) fn validate(label: Option<String>) -> Result<Option<String>, Error>
```

Rejects (AC #3, doc-2 §4) a label that contains `/`, or that is made only of digits (at
least one digit — an empty string is neither; it is left to a later task to decide
whether an empty label is meaningful at all). `None` always passes through. Every setter
calls this one function, so there is exactly one place the rule lives:
`Session::set_label`, `Tab::set_label`, `Pane::set_label` call it directly;
`Tree::new_session` and `Tree::new_tab` call it before minting any id and then set the
already-validated label with `.expect("validated above")`, so a bad label never consumes
an id (consistent with the existing comment in `new_tab`/`new_pane`).

These two bans are also why the match order in `target.rs` never has to choose between
label and position for the same string: no label can be all-digits, so a numeric part can
never match kind 2, and a position always gets its turn (ties AC #3 to AC #1).

### `target.rs`: the generic resolver

```rust
pub struct Candidate { pub id: String, pub label: Option<String> }

enum Resolution<Id> { Found(Id), NotFound(Vec<Candidate>), Ambiguous(Vec<Candidate>) }

fn resolve<'a, T, Id: std::fmt::Display + Copy>(
    part: &str,
    items: impl IntoIterator<Item = &'a T>,
    id_of: impl Fn(&T) -> Id,
    label_of: impl Fn(&T) -> Option<&str>,
) -> Resolution<Id>
```

Takes an iterator, not a slice: `Tab::panes()` returns `&[Pane]` today, but TASK-1.36
replaces the flat `Vec<Pane>` with a layout tree, after which that accessor is very
likely to hand out leaves through an iterator rather than a contiguous slice. Collecting
once into a `Vec<&T>` inside `resolve` (needed anyway, for position-by-index and for the
full candidate list on a miss) means nothing here has to change when that accessor's
return type does — only the call site passing `tab.panes()` does, and only if the
accessor is renamed, not if its return type changes.

Match order, four passes over the collected `Vec<&T>`, each only run if the previous
pass's candidate set was empty (R10's "stopping at the first kind that matches" — read as
*the first kind that matches at all*, i.e. yields one *or more* candidates, not "exactly
one": R10 also says "An ambiguous target is an error, never a guess", so an ambiguous
label must stop and report itself ambiguous rather than falling through to position or
suffix in search of a single answer):

1. Full identifier: `id_of(item).to_string() == part`, exact (case-sensitive: the spec
   grants case-insensitivity only to labels, never to identifiers, so matching a 7-char
   hex string as a 7-char hex string with different case is deliberately not supported).
2. Label, ignoring case: `label_of(item).map(str::to_lowercase) == Some(part.to_lowercase())`
   (`to_lowercase`, not `eq_ignore_ascii_case`, so a non-ASCII label still folds
   correctly; nothing in the spec restricts labels to ASCII).
3. Position: `part` parses as `usize`, `position >= 1`, `items.get(position - 1)`.
   One-based, in `items`' own order — see the position note below.
4. Identifier suffix, `part.len() >= 4`: `id_of(item).to_string().ends_with(part)`,
   case-sensitive for the same reason as kind 1.

Zero matches at the end of all four passes is `NotFound`, carrying every item at that
level as a `Candidate` (so a caller can show "did you mean one of these"). More than one
match at whichever kind stopped the search is `Ambiguous`, carrying just that matched
subset. `Candidate.label` comes from `label_of`; `Candidate.id` from `id_of(..).to_string()`.

**Position is my interpretation, flagged for TASK-1.36 to revisit.** doc-15 gives ordered
tabs (R8) and ordered sessions (`Tree::move_session`), so "position" is unambiguous for
those two levels: `Session::tabs()`'s and `Tree::sessions()`'s own order. For a pane, R95
makes a tab's layout a *tree*, and nothing in doc-15 says panes have an order at all. This
plan resolves pane position against `Tab::panes()`'s current order (creation order, by
the doc comment on that method today). TASK-1.36 is explicitly expected to look at this
when it builds the layout tree: either `panes()` keeps meaning "panes in some stable,
walkable order" (and this code needs no change beyond what the iterator-vs-slice choice
above already absorbs), or position for a pane needs its own rule once panes live in a
tree. I am not resolving that now; I am naming it so the next task's plan has to.

Three thin wrappers call `resolve` and turn its `Resolution` into `Result<Id, Error>`,
building `Error::TargetNotFound`/`Error::TargetAmbiguous` on the two failing arms:

```rust
impl Tree   { pub fn resolve_session(&self, part: &str) -> Result<SessionId, Error> }
impl Session{ pub fn resolve_tab(&self, part: &str) -> Result<TabId, Error> }
impl Tab    { pub fn resolve_pane(&self, part: &str) -> Result<PaneId, Error> }
```

And two composed, path-level methods on `Tree` — these are the task's headline deliverable
("resolve ... from a path", doc-2 §4's `[session/][tab/]pane` grammar), handling the
optional leading parts by falling back to a caller-given default id rather than any
notion of "current" the kernel tracks itself:

```rust
impl Tree {
    /// `path` is "[session/]tab" (doc-2 §4). `default_session` is used when
    /// the session part is left out.
    pub fn resolve_tab_path(&self, path: &str, default_session: SessionId)
        -> Result<TabId, Error>

    /// `path` is "[session/][tab/]pane". `default_session` and `default_tab`
    /// are used for parts left out.
    pub fn resolve_pane_path(&self, path: &str, default_session: SessionId, default_tab: TabId)
        -> Result<PaneId, Error>
}
```

Splitting `path` on `/` gives 1, 2 or 3 segments for `resolve_pane_path` (1 for
`resolve_tab_path`, 0 or wider is malformed). Anything else — 0, or more than 3 for a
pane, more than 2 for a tab, any empty segment (`"a//b"`, `"/a"`, `"a/"`) — is
`Error::InvalidTargetPath(String)`, not `NotFound`/`Ambiguous`: doc-2 §6 draws that line
itself (status 2, bad usage, vs. status 3, matched nothing or more than one), and an
empty segment can never come from a real id, label, position or suffix, so treating it as
"no candidates matched" would misreport a malformed path as an ordinary miss.

For each present segment, the matching `resolve_session`/`Session::resolve_tab` call runs;
for an absent leading segment, the given default id is used as-is (it is already a
specific id, needing no matching). `SessionNotFound`/`TabNotFound` (already existing
variants) cover a `default_session`/`default_tab` that no longer exists.

### `error.rs`

```rust
InvalidLabel(String),
InvalidTargetPath(String),
TargetNotFound { part: String, candidates: Vec<Candidate> },
TargetAmbiguous { part: String, candidates: Vec<Candidate> },
```

With matching `Display` arms (one line each, listing `len(candidates)` or the ids). Drop
`#[derive(..., Copy)]` from `Error` — verified against every current use (`tree.rs`,
`session.rs`, `tab.rs`, their tests): all `?`, `.ok_or(...)`, `.expect(...)` or
`assert_eq!`, none needs `Error` to be `Copy`.

## Order of work

1. `label.rs`: `validate`, with its own unit tests (AC #3's clauses).
2. `error.rs`: the four new variants, `Display` arms, drop `Copy`.
3. `pane.rs`, `tab.rs`, `session.rs`: `set_label` → `Result`, wired to `label::validate`;
   fix the existing tests in each file to `.unwrap()`.
4. `tree.rs`: `new_session` → `Result<SessionId, Error>`, `new_tab` propagates; fix every
   existing test call site (`tree.rs`'s own tests, and anywhere else in the crate that
   calls either).
5. `target.rs`: `Candidate`, `Resolution`, `resolve`, then `Tree::resolve_session`,
   `Session::resolve_tab`, `Tab::resolve_pane`, then `Tree::resolve_tab_path`,
   `Tree::resolve_pane_path`. Unit tests alongside, per the mapping below.
6. `lib.rs`: export `Candidate`.
7. `just check` (format, lint, boundaries, licences, affected tests — the recipe names on
   `main` today; `just --list` confirms `check`/`test`/`boundaries` all still exist in
   this worktree, ahead of TASK-1.73's rename).

## Risks

- The breaking signature changes (`set_label`, `new_session`) are intentional and
  contained to `crates/kernel`, the only crate besides `sdk` that exists yet — no other
  crate's call sites exist to break.
- Position-for-a-pane (above) is a judgement call, not a spec requirement; named as a
  coordination point for TASK-1.36 rather than resolved here.
- `TargetNotFound`'s candidate list can be large (every session/tab/pane at that level).
  Nothing in R10 or doc-2 §4 limits it, so this plan does not either; a later task is
  free to cap it for display once a real client exists.

## Proof — which test proves which clause

AC #1 ("tried as a full identifier, then a label without regard to case, then a
position, then an identifier suffix of four or more characters, stopping at the first
kind that matches"), in `target.rs`'s unit tests, against `resolve` directly plus one
confirming test per public wrapper:

- `resolve_matches_a_full_identifier_before_any_other_kind`
- `resolve_matches_a_label_case_insensitively_when_no_identifier_matches`
- `resolve_matches_by_one_based_position_when_no_identifier_or_label_matches`
- `resolve_matches_an_identifier_suffix_of_four_or_more_characters`
- `resolve_never_tries_a_suffix_shorter_than_four_characters`
- `resolve_stops_at_label_even_when_an_ambiguous_label_match_would_also_match_by_suffix`
  (two candidates share a label that is also a valid 4+ char suffix of a third
  candidate's id — the label kind stops the search as ambiguous; the suffix kind is
  never reached)
- `tree_resolve_session_session_resolve_tab_and_tab_resolve_pane_each_use_the_same_order`
  (one pane/tab/session level each, confirming the wrappers are not reordering anything)

AC #2 ("a target that matches nothing, or more than one thing, is an error that lists
the candidates and does nothing"):

- `resolve_reports_not_found_with_every_candidate_when_nothing_matches`
- `resolve_reports_ambiguous_with_only_the_matched_candidates_when_more_than_one_matches`
- `tree_resolve_session_propagates_not_found` and `..._ambiguous` (two sessions sharing a
  label)
- `resolve_pane_path_rejects_a_path_with_the_wrong_number_of_segments` and
  `..._rejects_an_empty_segment` (the `InvalidTargetPath` boundary against doc-2 §6's
  status 2 vs. status 3 split — not literally AC #2, but the edge AC #2's wording raises)
- `resolve_tab_path_and_resolve_pane_path_fall_back_to_the_given_default_when_a_leading_part_is_left_out`,
  and a companion test where an explicit leading part is given and overrides the default
  (proves the composition, not just the per-level primitive)

AC #3 ("a label containing `/`, or made only of digits, is refused"), in `label.rs`'s
unit tests plus one test per setter so every way of setting a label is proven to go
through it:

- `validate_rejects_a_label_containing_a_slash`
- `validate_rejects_a_label_made_only_of_digits`
- `validate_accepts_an_ordinary_label_and_a_cleared_label`
- `session_set_label_rejects_an_invalid_label`
- `tab_set_label_rejects_an_invalid_label`
- `pane_set_label_rejects_an_invalid_label`
- `tree_new_session_rejects_an_invalid_label_and_mints_no_id`
- `tree_new_tab_rejects_an_invalid_label_and_mints_no_id`

Each of these runs under `just check` (step 7), recorded via `asdlc record-check` before
build begins and after each meaningful step, per the team's testing policy.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan checked against doc-15 (R10), doc-2 section 4 and doc-16 (R105) by @bruk.o: ok. The four kinds of match are tried in order and stop at the first that matches anything; a miss or an ambiguous match is an error listing candidates; one validate function is called by every way of setting a label, before any id is minted. Nothing new goes in the sdk: a target crosses the API as text, and mapping the kernel's error to a wire error is TASK-1.43's. For the engineer: what a pane's position means once a tab's panes are a tree of splits is not written anywhere; the plan uses the order the tab lists its panes in, and the orchestrator proposes that be the order of the layout tree's leaves, first child before second.

Check passed: `just check` exited 0 at 2026-11-15T03:50:53+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.38 on task-1.38-kernel-resolve-a-session-tab-or-pane at 1025d7d9c81d:

```text
ok      format check (0.1s)
ok      lint (14.3s)
ok      boundaries (0.1s)
ok      licences (0.6s)
skip    tests: this change touches no crate
ok      tool tests (0.5s)
ok      architecture (1.1s)
ok      recipe names (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-15T03:58:40+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.38 on task-1.38-kernel-resolve-a-session-tab-or-pane at 1025d7d9c81d with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (-p steg -p steg-kernel -p steg-server) (0.1s)
ok      tool tests (0.2s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```

Check passed: `just check` exited 0 at 2026-11-15T03:59:43+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.38 on task-1.38-kernel-resolve-a-session-tab-or-pane at de1cf332a7c1:

```text
ok      format check (0.1s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (-p steg -p steg-kernel -p steg-server) (0.2s)
ok      tool tests (1.0s)
ok      architecture (0.8s)
ok      recipe names (0.0s)
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-15T03:50:00Z

---

Plan approved by @bruk (218939872684, size-2): its worker may build it.

---
<!-- COMMENTS:END -->

---
id: TASK-1.35
title: 'Kernel: sessions, tabs and panes as one tree, with its rules'
status: In Progress
assignee:
  - '@bruk.w3'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T03:01:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.69
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-5
asdlc_branch: task-1.35-kernel-sessions-tabs-and-panes-as-one
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'f3e4e6873d40'
asdlc_plan_approved_size: size-3
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-15: R6, R8, R12, R26, R93, R94, R109. doc-16 R105: the kernel is logic only and may depend on the sdk without its optional parts; identifiers, the colour set and pane kinds cross the API, so they are defined in the sdk and used here, not written twice. This task adds those shapes to the sdk. Revise the plan to this description; the earlier plan was written before doc-16 existed.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Sessions and tabs can be created, labelled, reordered and closed, and each pane, tab and session has exactly one parent
- [ ] #2 Closing a tab's last pane closes the tab, and closing a session's last tab ends the session
- [ ] #3 A new session takes the next of the five colours not in use, and its colour can be changed
- [ ] #4 A pane has a kind, and nothing in the tree assumes it is a terminal
- [ ] #5 The crate does no I/O, and the boundaries check passes
- [ ] #6 When all five colours are in use, a new session takes the first colour again (R109)
- [ ] #7 Identifiers, the colour set and pane kinds are defined once, in the sdk (doc-16 R105)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Revised for doc-16 (the earlier plan assumed a dependency-free kernel; doc-16 settles that the kernel may depend on the sdk, and that identifiers, the colour set and pane kinds are sdk shapes, not kernel's own). This task now spans two crates: crates/sdk (new files only — TASK-1.69 scaffolds the empty package, its Cargo.toml and its workspace membership; this task does not touch that scaffolding) and crates/kernel (as before, now consuming sdk's shapes instead of hand-rolling them).

Depends on TASK-1.69 landing first: crates/sdk must exist, be a workspace member, and boundaries.toml's `[steg-kernel]` table must already read `allow = ["steg-sdk"]` (doc-16's dependency table, which is TASK-1.69's own AC #4 to deliver) before this task's kernel-side Cargo.toml edit (below, needed before any kernel/ file in the order of work) can pass `just boundaries`. This plan does not edit boundaries.toml itself — that is TASK-1.69's file to own, per its own plan.

No serde, and no other new dependency, on crates/sdk in this task. doc-16 R106 allows the sdk a required dependency "for data and serialisation only", but TASK-1.69's AC #2 scaffolds it with "no behaviour yet, no I/O" and (going by its own plan) no dependencies declared. Wire serialisation of these shapes is TASK-1.43's concern (the API description) and decision-2's (the wire protocol); this task defines the shapes as plain Rust (Debug/Clone/Copy/Eq/Hash/Ord/Display derives only) and leaves adding serde, if it's added at all, to whichever task actually puts a shape on the wire. Flagged for the engineer: this is a scope call, not a rule doc-16 states outright.

Files:

In crates/sdk/src/ (new, added to the package TASK-1.69 scaffolds):
- ids.rs — SessionId, TabId, PaneId. Each gets a public way to be built from a raw value (e.g. `fn from_raw(value: u64) -> Self`), because the sdk decodes these off the wire; that same constructor is how the kernel mints one (below), not a separate kernel-only path. No minting sequence lives here — minting is server-side behaviour, not a shape (R105), and the sdk is the public interface every client uses (R101); a client never mints an identifier.
- colour.rs — Colour and Colour::ALL.
- pane_kind.rs — PaneKind.
- lib.rs — edited: `mod` and `pub use` lines added for the three files above, beside whatever frame TASK-1.69 already wrote.

In crates/kernel/src/ (new, as the earlier plan, minus the three shapes above):
- ids.rs — `Ids`, the minting sequence: one counter, three methods (`next_session`, `next_tab`, `next_pane`), each building its id through the sdk type's `from_raw`. This is the kernel's own behaviour (deciding the next id), built on a shape the sdk defines and exposes publicly for its own reason (decoding the wire) — not a constructor added to the sdk only so the kernel could mint.
- error.rs — the crate's Error enum.
- colour.rs — the kernel's own file: just the assignment rule (`next_colour`), not the `Colour` type (that's sdk's).
- pane.rs — Pane, now built on sdk::PaneKind and sdk::PaneId.
- tab.rs — Tab.
- session.rs — Session.
- tree.rs — Tree, the root, and the cascading close.
- lib.rs — edited: drop the placeholder test, add `mod`/`pub use` lines only.

Cargo.toml:
- crates/kernel/Cargo.toml — add `steg-sdk = { path = "../sdk" }` under `[dependencies]`, default features only (no feature list), so AC #4 of TASK-1.69 ("the kernel may depend on the sdk without its optional parts") holds by construction: the local-socket provider (R103) is off unless asked for, so a plain path dependency should suffice. Flagged: this plan assumes TASK-1.69 makes that feature non-default; if TASK-1.69 lands with it default-on, this line needs `default-features = false` and this plan is wrong about why a plain dependency is enough — check TASK-1.69's actual Cargo.toml for crates/sdk before writing this line.

Order of work (each step compiles and is tested before the next):

1. sdk/ids.rs. `SessionId`, `TabId`, `PaneId`: each a newtype over u64, Copy/Clone/Eq/PartialEq/Hash/Ord/PartialOrd/Debug/Display. Each gets a public `fn from_raw(value: u64) -> Self` — the sdk's own reason for this is decoding one off the wire, not so the kernel has a way in, though the kernel uses the same path (step 4). Display is a fixed-width hex string (e.g. 8 hex digits), long enough that TASK-1.38's "suffix of four or more characters" is meaningful. Unit tests (in sdk): `from_raw` round-trips through the raw value; Display is fixed-width; distinct raw values compare unequal.

2. sdk/pane_kind.rs. `PaneKind` enum: one variant, `Terminal`, for v1 (R12, doc-1 out-of-scope: "pane kinds other than terminals... R12 keeps the door open"). Plain enum, no `#[non_exhaustive]` — every consumer (kernel, server, app, cli) is workspace-internal, so a later variant should force every match site to be revisited (rust.md), the same reasoning as before doc-16, just now about a type that happens to live in a different crate of the same repository.

3. sdk/colour.rs. `Colour` enum: `Violet, Pink, Orange, Blue, Green`, in that fixed order (R26; matches doc-10 R26's wording and TASK-1's implementation note). `Colour::ALL: [Colour; 5]` in that order. No selection logic here — that's kernel's (step 6), since picking a colour for a session is domain behaviour, not part of the shape.

4. kernel/ids.rs. `Ids`: a small sequence generator — one counter, three minting methods (`next_session`, `next_tab`, `next_pane`), each building its id through the matching sdk type's `from_raw`. Minting is server-side behaviour (deciding the next id), not a shape, so it lives here and not in the sdk (doc-16 R105; a client never mints an identifier, and the sdk is the public interface every client uses, R101). `Tree` (step 10) holds one `Ids` and uses it for every mint, so an id only ever names something actually being created. Unit tests: mints distinct, increasing values across all three kinds from one counter.

5. kernel/error.rs. One crate-wide `Error` enum, hand-written (no thiserror; the kernel's own dependency stays limited to sdk): `SessionNotFound(sdk::SessionId)`, `TabNotFound(sdk::TabId)`, `PaneNotFound(sdk::PaneId)`, `PositionOutOfRange { len: usize, position: usize }`. Manual `Display` and `impl std::error::Error`. Every fallible public fn documents `# Errors` (missing_errors_doc). Later kernel tasks (1.36's split/resize/swap, 1.38's ambiguous/not-found target) are expected to add variants to this same enum rather than defining their own — note this for the engineer, since every later kernel task touches it.

6. kernel/colour.rs. `pub(crate) fn next_colour(in_use: impl IntoIterator<Item = sdk::Colour>) -> sdk::Colour`: scan `sdk::Colour::ALL` in order, return the first not in `in_use`; if none is free, return `sdk::Colour::ALL[0]` (R109, now a confirmed rule, not a guess — the engineer decided this, so there is no open question to flag here any more). Unit tests: no sessions in use → Violet; Violet and Pink in use → Orange; all five in use → Violet again (R109, exactly).

7. kernel/pane.rs. `Pane { id: sdk::PaneId, label: Option<String>, kind: sdk::PaneKind }`. `fn set_label(&mut self, label: Option<String>)`. No validation of the label's text (see the open note on labels below) — plain `Option<String>`. No `program` or `status` field: TASK-1.39/1.40/1.41 add those to this same struct. Unit test: a pane is created with kind `sdk::PaneKind::Terminal` and an unset label; `set_label` changes it.

8. kernel/tab.rs. `Tab { id: sdk::TabId, label: Option<String>, panes: Vec<Pane> }`, panes in creation order (a placeholder for TASK-1.36's layout tree, not yet geometry). Methods:
   - `fn add_pane(&mut self, id: sdk::PaneId, kind: sdk::PaneKind) -> &mut Pane` — takes an already-minted id; `Tab` mints nothing itself (`Tree` does, via its `Ids`).
   - `fn close_pane(&mut self, id: sdk::PaneId) -> Result<PaneClosed, Error>` where `PaneClosed { pane: sdk::PaneId, tab_now_empty: bool }` (R94: "a tab has at least one pane"; removing the last one reports `tab_now_empty: true` and leaves the empty tab in place for `Tree` to close).
   - `fn set_label(&mut self, label: Option<String>)`.
   - `fn panes(&self) -> &[Pane]`, `fn pane(&self, id: sdk::PaneId) -> Option<&Pane>`, `fn pane_mut(&mut self, id: sdk::PaneId) -> Option<&mut Pane>` — read/write access TASK-1.36 (layout), TASK-1.37 (focus) and TASK-1.38 (resolution) build on directly.
   Unit tests: closing a pane that isn't the only one leaves `tab_now_empty: false`; closing the only pane reports `tab_now_empty: true`; closing an id the tab doesn't hold is `Error::PaneNotFound`.
   Note for TASK-1.36: this flat `Vec<Pane>` is a placeholder for R95's layout tree. 1.36 is expected to replace it with a tree whose leaves hold `Pane` values directly (one source of truth for which panes exist), not a tree of `PaneId`s kept beside this vector — two structures that must agree by convention is exactly what rust.md's "illegal states should not be representable" forbids. (If R95's layout shape turns out to "cross the API" the way doc-16 R105 means it, that tree type may belong in sdk too — 1.36's own plan should settle that against doc-16, not inherit an assumption from this one.)

9. kernel/session.rs. `Session { id: sdk::SessionId, label: Option<String>, colour: sdk::Colour, tabs: Vec<Tab> }`, tabs in order (R8). Methods:
   - `fn add_tab(&mut self, id: sdk::TabId, first_pane: sdk::PaneId) -> &mut Tab` — a tab is never created empty (R94), so this takes its first pane's id too and the `Tab` is constructed with exactly one pane already in it.
   - `fn close_tab(&mut self, id: sdk::TabId) -> Result<TabClosed, Error>` where `TabClosed { tab: sdk::TabId, session_now_empty: bool }` (mirrors `Tab::close_pane`; R94's session half).
   - `fn set_label(&mut self, label: Option<String>)`, `fn set_colour(&mut self, colour: sdk::Colour)` (AC #3's "can be changed").
   - `fn move_tab(&mut self, id: sdk::TabId, to_position: usize) -> Result<(), Error>` (R8 "reordered"; `Error::PositionOutOfRange` if out of range).
   - `fn tabs(&self) -> &[Tab]`, `fn tab(&self, id: sdk::TabId) -> Option<&Tab>`, `fn tab_mut(&mut self, id: sdk::TabId) -> Option<&mut Tab>`.
   Unit tests: closing the only tab reports `session_now_empty: true`; closing one of several does not; `move_tab` reorders and rejects an out-of-range position.

10. kernel/tree.rs. `Tree { ids: Ids, sessions: Vec<Session> }` — the crate's root; the whole structure doc-15 calls "a server's tree" (R85) and later tasks call simply "the tree" (e.g. TASK-1.46's title).
   - `fn new() -> Self`.
   - `fn new_session(&mut self, label: Option<String>) -> sdk::SessionId` — colour is `colour::next_colour` over the colours of sessions currently in `self.sessions` (AC #3, R109); mints the session's id, its first tab's id and that tab's first pane's id via `self.ids`, and builds the Session/Tab/Pane in one call so R93/R94 are never false even for a moment.
   - `fn close_session(&mut self, id: sdk::SessionId) -> Result<(), Error>` — removes the session outright (no cascade above a session).
   - `fn close_tab(&mut self, id: sdk::TabId) -> Result<TabClosed, Error>` — finds the owning session, calls `Session::close_tab`; if `session_now_empty`, also removes the session from `self.sessions`.
   - `fn close_pane(&mut self, id: sdk::PaneId) -> Result<ClosedPane, Error>` where `ClosedPane { pane: sdk::PaneId, tab: Option<sdk::TabId>, session: Option<sdk::SessionId> }` (`tab`/`session` are `Some` only when the cascade also closed them) — finds the owning tab, calls `Tab::close_pane`; if `tab_now_empty`, calls `close_tab` on itself and folds its result in. This is rust.md's "core and shell" decision function: the server (a different crate, built later) turns the `Some` fields into R38's close events; this crate does no I/O to emit them.
   - `fn move_session(&mut self, id: sdk::SessionId, to_position: usize) -> Result<(), Error>` (R6 "reordered").
   - `fn sessions(&self) -> &[Session]`, `fn session(&self, id: sdk::SessionId) -> Option<&Session>`, `fn session_mut(&mut self, id: sdk::SessionId) -> Option<&mut Session>`.
   Unit tests: `new_session` gives distinct colours to the first five sessions created, in R26's order, and the sixth repeats the first (R109, exercised here at the `Tree` level too, not just in step 6's narrower test); closing a session's only tab's only pane ends the session (`ClosedPane` has both `tab` and `session` set) and `sessions()` no longer lists it; closing one of several tabs, or one of several panes, cascades no further than it should; `move_session`/`move_tab` reorder and reject bad positions; a not-found id at any level is the matching `Error` variant.

11. kernel/lib.rs. Replace the current body with `mod` declarations for the seven kernel files above, `pub use` for every public type and function named here, and the crate doc comment edited to say the kernel's shapes now come from `steg-sdk`. Drop the `#[cfg(test)] mod tests` placeholder — it existed only because the crate had nothing of its own to test, and now it does.

What the three dependent tasks get from this, by name:
- TASK-1.36 (layout): `Tab.panes: Vec<Pane>` (to replace with the layout tree, panes at its leaves — see the note in step 8, and the open question there about whether that tree is itself an sdk shape), `Pane`, `sdk::PaneId`, `Error` (to extend with its own refusal variants, e.g. for the 20x4 floor).
- TASK-1.37 (focus): `Session`, `Tab`, `Tab::panes`/`pane`/`pane_mut`, `Session::tabs`/`tab`/`tab_mut`, to add `current_tab: sdk::TabId` on `Session` and `current_pane: sdk::PaneId` on `Tab`, plus the setters. These fields are deliberately not added here.
- TASK-1.38 (target resolution): `Tree::sessions`/`session`, `Session::tabs`/`tab`, `Tab::panes`/`pane`, each sdk id type's `Display` (for "full identifier" and the "suffix of four or more characters" match), and the `position` ordering already implicit in `tabs()`/`panes()` returning a slice. Labels stay plain `Option<String>` here; 1.38 either validates at the point it reads a label for matching, or introduces its own validating `Label` type (and decides, per doc-16, whether that type belongs in sdk as something that "crosses the API" — this plan does not pre-empt that).

Open questions / risks:
- Sequencing: this task cannot be built until TASK-1.69 merges and crates/sdk exists with its workspace membership and boundaries.toml entry in place (see the top of this plan).
- The sdk/kernel feature split for R103's optional local-socket provider is assumed, not confirmed against TASK-1.69's actual Cargo.toml (see the Cargo.toml note above) — check before step 4, the first kernel/ file that needs the sdk dependency in place.
- Whether sdk shapes carry serde now or later is this plan's scope call (see the "No serde" note above), not a decision on record.
- The colour wrap-around (R109) is no longer open — implemented and tested directly per the engineer's decision.

Proof:
- `just check` from the repo root (fmt, lint, `just boundaries` — AC #5's boundaries check — licence check, the affected tests via `just test`, and the tools' own tests). Record its output on the task.
- Unit tests sit in `#[cfg(test)]` modules beside the logic they prove (sdk's shapes tested in sdk, kernel's behaviour tested in kernel), per testing-rust.md; no doubles, since there's nothing outside either crate to fake.
- AC #7 ("defined once, in the sdk") is proven by construction and by review, not a new automated check: `SessionId`, `TabId`, `PaneId`, `Colour` and `PaneKind` are declared only under crates/sdk/src/, and every kernel file that needs one writes `use steg_sdk::...` rather than defining its own.
- `crates/kernel/Cargo.toml` gains exactly one dependency (`steg-sdk`, default features); nothing else in the kernel's dependency graph changes, confirming AC #5's "the crate does no I/O" still holds — sdk itself does none either (R102).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan held by @bruk.o, not sent for approval: it hand-rolls ids, colours and errors in the kernel on the assumption that the kernel depends on nothing. The engineer is settling whether the kernel may depend on an SDK with no I/O, in which case ids, the colour set and pane kinds are defined once there. To be revised once the architecture spec is on the board. Open for the engineer: which colour a sixth session takes when all five are in use (the plan proposes wrapping to violet).

Revised plan checked against doc-15 and doc-16 by @bruk.o: ok. Id types, the colour set and pane kinds are in the sdk; minting, colour choice (R109) and the tree are in the kernel. It names the types the layout, focus and target tasks build on. It cannot be built until TASK-1.69 merges.

Check passed: `just check` exited 0 at 2026-11-15T03:01:33+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.35 on task-1.35-kernel-sessions-tabs-and-panes-as-one at 1d256ad14af0:

```text
ok      format check (0.1s)
ok      lint (13.9s)
ok      boundaries (0.1s)
ok      licences (0.6s)
skip    tests: this change touches no crate
ok      tool tests (0.5s)
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-15T02:41:00Z

---

Plan approved by @bruk (f3e4e6873d40, size-3): its worker may build it.

---
<!-- COMMENTS:END -->

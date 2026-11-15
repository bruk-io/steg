---
id: TASK-1.35
title: 'Kernel: sessions, tabs and panes as one tree, with its rules'
status: In Progress
assignee:
  - '@bruk.w3'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T02:31:00Z'
labels:
  - size-3
milestone: m-0
dependencies: []
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-5
asdlc_branch: task-1.35-kernel-sessions-tabs-and-panes-as-one
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-15: R6, R8, R12, R26, R93, R94. The kernel is logic only (the repository's architecture rule): data in, data out, no I/O.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Sessions and tabs can be created, labelled, reordered and closed, and each pane, tab and session has exactly one parent
- [ ] #2 Closing a tab's last pane closes the tab, and closing a session's last tab ends the session
- [ ] #3 A new session takes the next of the five colours not in use, and its colour can be changed
- [ ] #4 A pane has a kind, and nothing in the tree assumes it is a terminal
- [ ] #5 The crate does no I/O, and the boundaries check passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Scope: crates/kernel only. Builds the frame crates/kernel/src/lib.rs already names ("session, tab and pane state"). No dependency is added to crates/kernel/Cargo.toml; boundaries.toml's `allow = []` for steg-kernel stays as it is. No serde: R101's steg library is where wire types live and map onto these by hand (decision-1/doc-8 R101); kernel stays dependency-free. This is an assumption the plan surfaces for the engineer to confirm, since it binds TASK-1.43's design.

Files (all new, under crates/kernel/src/):
- ids.rs — SessionId, TabId, PaneId.
- colour.rs — Colour and the five-colour rule.
- error.rs — the crate's Error enum.
- pane.rs — PaneKind, Pane.
- tab.rs — Tab.
- session.rs — Session.
- tree.rs — Tree, the root, and the cascading close.
- lib.rs — edited: drop the placeholder test (the doc comment's "no behaviour of its own yet" is no longer true), add `mod` and `pub use` lines only, per rust.md.

Order of work (each step compiles and is tested before the next):

1. ids.rs. `SessionId`, `TabId`, `PaneId`: each a newtype over u64, Copy/Clone/Eq/PartialEq/Hash/Ord/PartialOrd/Debug/Display. No public constructor — only `Tree` mints one, from one counter it holds, so an id always names something that was really created. Display is a fixed-width hex string (e.g. 8 hex digits) long enough that TASK-1.38's "suffix of four or more characters" is meaningful. Unit tests: distinct ids compare unequal, Display is fixed-width.

2. error.rs. One crate-wide `Error` enum, hand-written (no thiserror — zero deps): `SessionNotFound(SessionId)`, `TabNotFound(TabId)`, `PaneNotFound(PaneId)`, `PositionOutOfRange { len: usize, position: usize }` (for the reorder/move methods below). Manual `Display` and `impl std::error::Error`. Every fallible public fn documents `# Errors` (missing_errors_doc). Later kernel tasks (1.36's split/resize/swap, 1.38's ambiguous/not-found target) are expected to add variants to this same enum rather than defining their own — note this for the engineer, since every later kernel task touches it.

3. colour.rs. `Colour` enum: `Violet, Pink, Orange, Blue, Green`, in that fixed order (R26, matches TASK-1's implementation note and doc-10 R26 wording). `Colour::ALL: [Colour; 5]` in that order. `pub(crate) fn next_colour(in_use: impl IntoIterator<Item = Colour>) -> Colour`: scan `ALL` in order, return the first not in `in_use`. A pure function, no cursor, no stored history — the plain reading of R26's "takes the next colour not in use". Unit tests: no sessions in use → Violet; Violet and Pink in use → Orange; all five in use → falls back to `ALL[0]` (Violet) — see the open question below, this is the proposed default, not a confirmed rule.

4. pane.rs. `PaneKind` enum: one variant, `Terminal`, for v1 (R12, doc-1 out-of-scope: "pane kinds other than terminals... R12 keeps the door open"). Plain enum, no `#[non_exhaustive]` — server and app are workspace-internal, so a later variant should force every match site to be revisited (rust.md). `Pane { id: PaneId, label: Option<String>, kind: PaneKind }`. `fn set_label(&mut self, label: Option<String>)`. No validation of the label's text here (see the open note on labels below) — plain `Option<String>`. No `program` or `status` field: TASK-1.39/1.40/1.41 add those to this same struct. Unit test: a pane is created with kind `Terminal` and an unset label; `set_label` changes it.

5. tab.rs. `Tab { id: TabId, label: Option<String>, panes: Vec<Pane> }`, panes in creation order (R8's "ordered tabs" is about tabs within a session; a tab's own pane order here is a placeholder for TASK-1.36's layout tree, not yet geometry). Methods:
   - `fn add_pane(&mut self, id: PaneId, kind: PaneKind) -> &mut Pane` — takes an already-minted id; `Tab` mints nothing itself, so id-minting stays in one place (`Tree`, below).
   - `fn close_pane(&mut self, id: PaneId) -> Result<PaneClosed, Error>` where `PaneClosed { pane: PaneId, tab_now_empty: bool }` (R94: "a tab has at least one pane"; removing the last one reports `tab_now_empty: true` and leaves the empty tab in place for the caller — `Tree`, below — to close).
   - `fn set_label(&mut self, label: Option<String>)`.
   - `fn panes(&self) -> &[Pane]`, `fn pane(&self, id: PaneId) -> Option<&Pane>`, `fn pane_mut(&mut self, id: PaneId) -> Option<&mut Pane>` — read/write access TASK-1.36 (layout), TASK-1.37 (focus) and TASK-1.38 (resolution) build on directly.
   Unit tests: closing a pane that isn't the only one leaves the tab with `tab_now_empty: false`; closing the only pane reports `tab_now_empty: true`; closing an id the tab doesn't hold is `Error::PaneNotFound`.

   Note for TASK-1.36: this flat `Vec<Pane>` is a placeholder for R95's layout tree. 1.36 is expected to replace it with a tree whose leaves hold `Pane` values directly (one source of truth for which panes exist), not a tree of `PaneId`s kept beside this vector — two structures that must agree by convention is exactly what rust.md's "illegal states should not be representable" forbids.

6. session.rs. `Session { id: SessionId, label: Option<String>, colour: Colour, tabs: Vec<Tab> }`, tabs in order (R8). Methods:
   - `fn add_tab(&mut self, id: TabId, first_pane: PaneId) -> &mut Tab` — a tab is never created empty (R94), so this takes the id of its first pane too and the Tab is constructed with exactly one pane already in it.
   - `fn close_tab(&mut self, id: TabId) -> Result<TabClosed, Error>` where `TabClosed { tab: TabId, session_now_empty: bool }` (mirrors Tab::close_pane; R94's session half).
   - `fn set_label(&mut self, label: Option<String>)`, `fn set_colour(&mut self, colour: Colour)`.
   - `fn move_tab(&mut self, id: TabId, to_position: usize) -> Result<(), Error>` (R8 "reordered"; `Error::PositionOutOfRange` if `to_position > tabs.len() - 1`).
   - `fn tabs(&self) -> &[Tab]`, `fn tab(&self, id: TabId) -> Option<&Tab>`, `fn tab_mut(&mut self, id: TabId) -> Option<&mut Tab>`.
   Unit tests: closing the only tab reports `session_now_empty: true`; closing one of several does not; move_tab reorders and rejects an out-of-range position.

7. tree.rs. `Tree { next_id: u64, sessions: Vec<Session> }` — the crate's root; the whole structure doc-15 calls "a server's tree" (R85) and later tasks call simply "the tree" (e.g. TASK-1.46's title). One counter mints every id of every kind, so ids are unique across the whole tree, not just within a parent — needed for TASK-1.38's "full identifier" and "suffix" matching to be unambiguous regardless of where in the tree they're tried.
   - `fn new() -> Self`.
   - `fn new_session(&mut self, label: Option<String>) -> SessionId` — colour is `colour::next_colour` over the colours of sessions currently in `self.sessions` (AC #3); mints the session's id, its first tab's id and that tab's first pane's id, and builds the Session/Tab/Pane in one call so R93/R94's "every session has at least one tab, every tab at least one pane" is never false even for a moment.
   - `fn close_session(&mut self, id: SessionId) -> Result<(), Error>` — removes the session outright (there is no cascade above a session).
   - `fn close_tab(&mut self, id: TabId) -> Result<TabClosed, Error>` — finds the owning session, calls `Session::close_tab`; if `session_now_empty`, also removes the session from `self.sessions` (R94's session half, applied).
   - `fn close_pane(&mut self, id: PaneId) -> Result<ClosedPane, Error>` where `ClosedPane { pane: PaneId, tab: Option<TabId>, session: Option<SessionId> }` (`tab`/`session` are `Some` only when the cascade also closed them) — finds the owning tab, calls `Tab::close_pane`; if `tab_now_empty`, calls `close_tab` on itself and folds its result in. This is the "data in, data out" decision function rust.md's "core and shell" section asks for: the server (crate/server, not this task) turns the `Some` fields into the R38 close events; this crate does no I/O to emit them.
   - `fn move_session(&mut self, id: SessionId, to_position: usize) -> Result<(), Error>` (R6 "reordered").
   - `fn sessions(&self) -> &[Session]`, `fn session(&self, id: SessionId) -> Option<&Session>`, `fn session_mut(&mut self, id: SessionId) -> Option<&mut Session>`.
   Unit tests: `new_session` gives distinct colours to the first five sessions created, in R26's order; closing a session's only tab's only pane ends the session (`ClosedPane` has both `tab` and `session` set) and `sessions()` no longer lists it; closing one of several tabs, or one of several panes, cascades no further than it should; `move_session`/`move_tab` reorder and reject bad positions; a not-found id at any level is the matching `Error` variant.

8. lib.rs. Replace the current body with `mod` declarations for the seven files above, `pub use` for every public type and function named here, and the crate doc comment kept (editing its "behaviour lands here starting with TASK-1.6" line, since it now has). Drop the `#[cfg(test)] mod tests` placeholder — it existed only because the crate had nothing of its own to test, and now it does.

What the three dependent tasks get from this, by name:
- TASK-1.36 (layout): `Tab.panes: Vec<Pane>` (to replace with the layout tree, panes at its leaves — see the note in step 5), `Pane`, `PaneId`, `Error` (to extend with its own refusal variants, e.g. for the 20x4 floor).
- TASK-1.37 (focus): `Session`, `Tab`, `Tab::panes`/`pane`/`pane_mut`, `Session::tabs`/`tab`/`tab_mut` to add `current_tab: TabId` on `Session` and `current_pane: PaneId` on `Tab`, plus the setters. These fields are deliberately not added here.
- TASK-1.38 (target resolution): `Tree::sessions`/`session`, `Session::tabs`/`tab`, `Tab::panes`/`pane`, each id type's `Display` (for "full identifier" and the "suffix of four or more characters" match), and the `position` ordering already implicit in `tabs()`/`panes()` returning a slice. Labels stay plain `Option<String>` here (see below); 1.38 either validates at the point it reads a label for matching, or introduces its own validating `Label` type — this plan does not build one, since "a label containing `/`, or made only of digits, is refused" is 1.38's own acceptance criterion, not this task's.

Open questions / risks to flag on the task before or alongside finishing it:
- The sixth concurrent session's colour (R26 says "the next colour not in use"; with five colours and six or more sessions open at once, none is free). Proposed default: wrap to `Colour::ALL[0]` (Violet). Not confirmed by any spec text or decision record — worth a line to Bruk, same as doc-10's "not yet confirmed" list.
- No serde in the kernel (see Scope, above) is this plan's assumption, not a decision on record. If wrong, it is a `boundaries.toml` edit plus a reason, which the plan deliberately avoids making alone.

Proof:
- `just check` from the repo root (fmt, lint, `just boundaries` — AC #5's boundaries check — licence check, the affected tests via `just test`, and the tools' own tests). Record its output on the task.
- Unit tests sit in `#[cfg(test)]` modules beside the logic they prove, per testing-rust.md; no doubles, since there's nothing outside the kernel to fake.
- `crates/kernel/Cargo.toml` and `boundaries.toml` are unchanged — confirms AC #5's "the crate does no I/O" by construction, not just by the boundaries tool.
<!-- SECTION:PLAN:END -->

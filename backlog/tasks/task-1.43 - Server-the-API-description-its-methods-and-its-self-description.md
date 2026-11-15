---
id: TASK-1.43
title: 'Server: the API description, its methods and its self-description'
status: In Progress
assignee:
  - '@bruk.w2'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T14:04:00Z'
labels:
  - size-4
milestone: m-0
dependencies:
  - TASK-1.36
  - TASK-1.37
  - TASK-1.38
  - TASK-1.69
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-13
asdlc_branch: task-1.43-server-the-api-description-its-methods
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-8: R36, R37; doc-16 R101. decision-1. The API is one value, and it lives in the sdk; the server dispatches from it and serves it unchanged.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every method the first release needs (doc-2, section 8) is defined once, in the description, with its parameters and documentation
- [ ] #2 The server validates and dispatches a request against the description, and rejects an unknown method or parameter
- [ ] #3 A client can ask for every method and for one method in full, and can call any method by name
- [ ] #4 `spikes/api-description/` is deleted
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Decision this plan commits to

AC #1 reads "Every method the first release needs (doc-2, section 8) is defined once,
in the description" — all 38 of doc-2 section 8's rows (not `version`/`completion`,
which that document itself calls "not an API method"), full stop. So every method is
**described now**. What differs is **dispatch**: a method the kernel can already serve
in full gets a real body; one it cannot (because a field its `returns` shape promises,
or an operation it needs, does not exist in the kernel yet — programs, status, the
terminal engine, clients, server lifecycle, events) gets a distinct `not_implemented_yet`
error naming the task that is expected to replace it, never a fabricated field. This
keeps decision-1's own worked example ("every method in the description has a body")
true as two separate, testable claims instead of one that would be false at 38:38 vs
~24 real bodies — every described method has a dispatch *arm* (routable), and no arm
exists for an undescribed method; which arms are real versus `not_implemented_yet` is
a second, explicit, tested list.

## The method table (doc-2 section 8, 38 rows)

Servable now = every field its `returns` shape needs, and every kernel operation it
calls, already exists after TASK-1.36/1.37/1.38/1.69. `server`, `event`, `client` and
`pane.send/capture/wait` need the terminal engine, program/status, clients or server
lifecycle (TASK-1.39–1.42, 1.44, 1.46–1.48) and get `not_implemented_yet`.

| Noun | Served now | Not yet (needs) |
|---|---|---|
| session | list, show, new, close, move, set, unset | focus (needs a client/window to address — TASK-1.42/1.46) |
| tab | new, close, move, set, unset, focus | list, show (title, shown_status — TASK-1.39, 1.41) |
| pane | split, close, move, set, unset, focus, resize, swap | list, show (title, cwd, program, exit_code, status — TASK-1.39–1.41); send, capture, wait (terminal engine — TASK-1.39/1.40) |
| server | — | show, start, stop (TASK-1.44) |
| event | — | watch (TASK-1.46) |
| client | — | list, show (TASK-1.42) |
| api | list, show, call | — (this task's own job) |

24 served, 14 `not_implemented_yet`. Two things fall out of "served" that are not full
behaviour, documented as such rather than hidden:
- `--force` on `*.close` is accepted and currently a no-op: the kernel has no concept of
  a running program yet, so nothing it can see is ever running. TASK-1.40 gives it teeth.
- `session.new --focus`/`--no-focus` is accepted and currently a no-op: there is no
  "current session" in doc-15, only `current_tab`/`current_pane`; what the flag really
  means (which client's window shows the new session) is R87's, and needs the client
  model (TASK-1.42) and the session-focus event (TASK-1.46) to do anything.
- `tab.new`/`pane.split --no-focus` DO have real, kernel-level meaning (R115) and are
  implemented for real: capture the previous current tab/pane, create, and if
  `focus=false` put it back, with no event in between (there is no eventing yet, so
  that half is structural, not separately provable here — TASK-1.46 is where
  "no focus change announced in between" becomes testable against a subscriber).

## Where validation and dispatch live, and why

- **Validation** (a request's method and parameter names, required-ness, and closed-set
  values against the description) is pure data-in data-out over `Description` and
  `Request` — no kernel, no I/O. It goes in **`steg-sdk`** (R101: "the protocol's
  logic"), so the same function the server uses for AC #2 is also what a future CLI
  (TASK-1.49) can use for doc-2 section 5's "the CLI checks the name before any
  request" without a second implementation.
- **Dispatch** (actually running an operation) needs `&mut Tree`, which the sdk may
  never depend on (R101/R105: the sdk has nothing of steg's). It goes in **`steg-server`**,
  which already depends on both `steg-kernel` and `steg-sdk` (R98: no client depends on
  the server, so putting it here costs nothing — the server is already the one place
  validation's result and the kernel meet).
- `dispatch` takes an explicit `Context { default_session, default_tab, default_pane }`
  alongside the `Request`, rather than reading anything off a connection. There is no
  connection yet (TASK-1.44/1.45), and the kernel's own target resolution
  (`Tree::resolve_pane_path` etc.) already takes its defaults as parameters, not from
  tracked state — `dispatch`'s signature mirrors that on purpose, so TASK-1.45 ("requests
  routed to the API") has exactly one thing left to do: build a `Context` from a
  connection's `STEG_SESSION`/`STEG_TAB`/`STEG_PANE` and pass it through unchanged.

## Shapes, and the wire question this task does not answer

Request/response values are `serde_json::Value` (`Request { method: String, params:
Map<String, Value> }`, `Response::Ok(Value) | Response::Error(ApiError)`), matching the
spike and decision-1's own format: this *is* the boundary decision-2 calls "control
payloads are JSON," not an internal shortcut past rust.md's "typed struct at a
boundary" rule — the typed struct is `Description`/`Method`/`Param`/etc., and `Value`
is what crosses into it. How `Request`/`Response` get onto an actual socket (framing,
the hello, streams) is TASK-1.44/1.45/1.70's, untouched here.

`events()` seeds doc-8 R38's M1 names now (so `--for <event>` has something to validate
against before TASK-1.49, per doc-2 section 5): `session.created/closed/focused`,
`tab.created/closed/focused`, `pane.created/closed/focused`,
`pane.cwd_changed/title_changed/bell/exited/status_changed` — 14 records, fields drawn
from doc-14's record tables where it has them. TASK-1.46 fills in behaviour; the names
and fields are fixed from here so nothing downstream guesses at them twice.

`api.show` with no `target` returns the whole `Description` (methods, events, errors,
api_version) in one call — my reading of doc-2 section 8's row for `api show`, which
doesn't mark its target required, used to satisfy decision-1's "serialized and served
as the answer to one call" (R37, for a Python plugin that can't import the Rust value)
without adding a verb doc-2 doesn't have (section 3: "Adding a verb... is a change to
this document"). Flagged for the engineer to confirm or overrule, not decided silently.

## Files that change

- `boundaries.toml`: add `serde` and `serde_json` to `[steg-sdk]`'s `allow`, with a
  reason under R106 (data/serialisation only); rewrite that table's `reason` line,
  which currently says "has nothing of steg's to depend on" and would be false. Add
  `serde_json` to `[steg-server]`'s `allow`, reason: building the response payload
  dispatch answers with.
- `crates/sdk/Cargo.toml`: add `serde = { version = "1", features = ["derive"] }`,
  `serde_json = "1"`.
- `crates/sdk/src/lib.rs`: `mod description; pub use description::{...};` (mod/pub use
  only, per rust.md); update the crate doc comment.
- `crates/sdk/src/description/mod.rs` (new): `mod`/`pub use` only.
- `crates/sdk/src/description/shapes.rs` (new): `Description`, `Version`, `MethodName`,
  `Method`, `Param`, `Role`, `ValueKind`, `Returns`, `Field`, `Event`, `ErrorRecord`,
  `ErrorCode` (ten variants: `BadRequest`, `NoSuchTarget`, `AmbiguousTarget`,
  `ProgramRunning`, `PositionOutOfRange`, `SplitTooSmall`, `NoSplitInDirection`,
  `PanesNotInSameTab`, `AlreadyInTab`, `NotImplementedYet`), each with serde derives;
  `Description::method/nouns/verbs_of`, `Param::flag/negated_flag`,
  `Method::target/program`. No serde derives added to the existing sdk types
  (`SessionId`, `Colour`, `PaneKind`, `Layout`...) — they cross the wire as
  `Value`/strings built by hand in dispatch, not by deriving on the domain types
  themselves, so nothing already on main is touched.
- `crates/sdk/src/description/request.rs` (new): `Request`, `Response`, `ApiError`,
  `validate(&Description, &Request) -> Result<(), ApiError>`.
- `crates/sdk/src/description/methods.rs` (new): `description() -> Description`, built
  from `session_methods()/tab_methods()/pane_methods()/server_methods()/
  client_methods()/api_methods()` (38 `Method` literals total, summaries/details/params
  from doc-2 sections 3, 5, 8 and decision-4), `events()` (14 records), `errors()` (10
  records with exit statuses per doc-2 section 6: `BadRequest`→2, `NoSuchTarget`→3,
  `AmbiguousTarget`→3, `PositionOutOfRange`→2, `ProgramRunning`→1, `SplitTooSmall`→1,
  `NoSplitInDirection`→1, `PanesNotInSameTab`→1, `AlreadyInTab`→1,
  `NotImplementedYet`→1).
- `crates/kernel/src/tree.rs`: add `Tree::set_tab_current_pane(&mut self, tab: TabId,
  pane: PaneId) -> Result<(), Error>` (sets only that tab's own current pane, no
  cascade to the session — unlike `focus_pane`) and `Tree::tab_of_pane(&self, pane:
  PaneId) -> Option<TabId>` (read-only lookup). Both are the one kernel addition this
  task needs: restoring a tab's focus after `pane.split --no-focus` without touching
  the session's `current_tab`, and finding a pane's tab to call `unfill_pane` for
  `pane.unset --zoomed`. Pure data, no event, no I/O — in scope for the kernel's own
  `CLAUDE.md`/`clippy.toml` as written.
- `crates/server/Cargo.toml`: add `serde_json`.
- `crates/server/src/lib.rs`: `mod dispatch; pub use dispatch::{dispatch, Context};`;
  update the crate doc comment (it currently says "behaviour lands starting with
  TASK-1.9" — this is the first behaviour).
- `crates/server/src/dispatch/mod.rs` (new): `Context`, `dispatch(tree: &mut Tree,
  request: &Request, context: Context) -> Response`, the top-level match by noun,
  `not_implemented_yet(name) -> ApiError`.
- `crates/server/src/dispatch/session.rs`, `tab.rs`, `pane.rs`, `api.rs` (new): one
  file per noun's served arms, each a plain function over `&mut Tree`.
- `crates/server/src/dispatch/kernel_error.rs` (new): `map_kernel_error(steg_kernel::Error)
  -> ApiError`, one arm per kernel `Error` variant.
- `crates/server/tests/dispatch.rs` (new, functional — a real `Tree`, no doubles, per
  testing-rust.md): the AC #2/#3 cases below.
- `spikes/api-description/` (deleted, AC #4): every file under it.

## Order of work

1. `boundaries.toml` + `crates/sdk/Cargo.toml`: add the two dependencies. `just
   check-boundaries` and `just test` (still a no-op crate) confirm the edit alone is
   clean before any behaviour is written.
2. `crates/sdk/src/description/shapes.rs`: the shape types and their derives. Unit
   test: one example `Method` round-trips through `serde_json`.
3. `crates/sdk/src/description/request.rs`: `Request`/`Response`/`ApiError`/`validate`.
   Port the spike's three validate tests (undeclared parameter, missing required,
   closed-set violation) and add "unknown method → `BadRequest`".
4. `crates/sdk/src/description/methods.rs`: all 38 methods, 14 events, 10 errors,
   assembled by `description()`. Tests: the method set is exactly doc-2 section 8's 38
   `(noun, verb)` pairs, no duplicates; every summary/details/param-summary is
   non-empty; no method declares a parameter named after a CLI-only flag (`json`,
   `quiet`, `verbose`, `no_color`, `no_input`, `help`, `host`, `autostart`); at most one
   `Target` and one `Program` role per method (ported from the spike); every error a
   method names has a record in `errors()`.
5. `crates/sdk/src/lib.rs`: wire the new module in.
6. `crates/kernel/src/tree.rs`: `set_tab_current_pane` and `tab_of_pane`, with unit
   tests — restoring a tab's current pane does not move the session's `current_tab`
   even when that tab is not the session's current one; `tab_of_pane` finds the right
   tab and returns `None` for an unknown pane.
7. `boundaries.toml` + `crates/server/Cargo.toml`: add `serde_json` to the server.
8. `crates/server/src/dispatch/`: `Context`, the top-level match, and the four
   per-noun files — 24 real arms, 14 `not_implemented_yet` arms each citing its task in
   a comment, `kernel_error.rs`'s full mapping. `pane.split`/`tab.new`'s `--no-focus`
   path uses step 6's additions; `pane.set/unset --zoomed` uses `fill_pane`/
   `unfill_pane` (`tab_of_pane` for `unfill_pane`'s `TabId`); `*.close`'s `force` and
   `session.new`'s `focus` are accepted and documented no-ops (see above).
9. `crates/server/src/lib.rs`: wire `dispatch` and `Context` in.
10. `crates/server/tests/dispatch.rs`: the AC #2/#3 cases (listed under Proof) plus two
    small end-to-end scenarios: split → set label → close a session through dispatch
    alone, and `tab.new` with `focus=false` leaving the original tab current.
11. Delete `spikes/api-description/`.
12. `just check` in full; fix what it finds; record the final run.

Steps 1–6 (the sdk's shapes, description and validation, with no server change) are one
reviewable unit that TASK-1.49 (the CLI) could build against on their own; steps 7–12
(dispatch, the kernel addition, error mapping, `--no-focus`, deletion) are the second.
Flagging this for the engineer to decide whether to split the task — not splitting it
myself.

## Risks

- **38 methods is a lot of literal data to get right by hand.** Mitigated by step 4's
  completeness test (exact match against a doc-2-derived list) and by grouping into
  per-noun functions, as the spike's own section 8 recommends staying readable to.
- **The served/not-implemented split is this plan's own reading of what "every field
  the shape promises" means**, not something doc-2/doc-14 states as a rule. Mitigated
  by the table above being explicit and reviewable against those documents directly,
  and by every `not_implemented_yet` arm citing its task in code, not just in the plan.
- **`Tree::set_tab_current_pane`/`tab_of_pane` are new kernel surface** outside what
  TASK-1.36–1.38 shipped. Kept deliberately minimal (pure data, no event) so TASK-1.46's
  eventing wraps them rather than replacing them.
- **`api.show` with no target answering the whole description** is a reading of doc-2,
  not an explicit instruction. Flagged above for the engineer; if overruled, only
  `dispatch/api.rs` and its tests change.
- **sdk's first-ever external dependency.** `just check` (licence, boundaries, lint) is
  run after step 1, not only at the end, to catch a licence or clippy surprise early —
  `Unicode-3.0` (reached through `serde_derive`'s `syn`) is already on `deny.toml`'s
  allow list, and `serde`/`serde_json` are already in `Cargo.lock` via other
  dependencies, so no new licence is expected, only confirmed.

## Proof

- AC #1: step 4's tests (exact 38-method set against doc-2 section 8; no empty
  summary/details/param-summary; no CLI-only-flag parameter; at most one target/program
  each); `just test` for `steg-sdk`.
- AC #2: `dispatch` tests for an unknown method, an undeclared parameter, a missing
  required parameter, and a word outside a closed set, each asserting `ApiError::code
  == BadRequest`; a completeness test that every described method has a dispatch arm
  and every arm is described (both directions, the two lists from the table above); a
  test that every `steg_kernel::Error` variant maps to an `ApiError`; the split/close
  end-to-end scenario; `just test` for `steg-server` and `steg-kernel`.
- AC #3: `api.list` returns all 38 methods once; `api.show <name>` returns one method
  in full (every param, its summary); `api.show` with no target returns the whole
  `Description`; `api.call` dispatches to the named method with its params and matches
  calling it directly; `api.call` of `api.call` is refused.
- AC #4: `spikes/api-description/` absent from `git -C <worktree> status`; `just check`
  passes with no reference to it (nothing outside `spikes/` depended on it per its own
  README).
- `just check` recorded in full at the end.

## For the task's notes (not this plan)

AC #4 deletes `spikes/api-description/`, but decision-1 sections 3 and 9 cite
`carry-one-method` and `measure.py` under that path as TASK-1.3's recorded evidence.
Deleting the directory leaves those references dangling. Decision text changes only
via a PR containing nothing else (the project's own rule) — not this task's to fix;
noting it for the orchestrator.
<!-- SECTION:PLAN:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-15T14:04:00Z

---

Plan written (phase 1). One open flag for the engineer, not decided silently: `api.show` with no target is read in this plan as returning the whole `Description` (decision-1's "served to anyone who asks... at the call"), since doc-2 section 8's row for `api show` doesn't mark its target required and adding a new verb would need a document change. Confirm or overrule.

Also noting for whoever takes this to review/build: AC #4 deletes `spikes/api-description/`, but decision-1 sections 3 and 9 cite `carry-one-method` and `measure.py` under that path as TASK-1.3's recorded evidence — deleting it leaves those references dangling. Not this task's to fix (decision text changes only via a PR containing nothing else); flagging so it isn't lost.

The plan also proposes a possible split at the sdk/server boundary (steps 1-6 vs 7-12) for the engineer to consider, without changing the task's size here.

---
<!-- COMMENTS:END -->

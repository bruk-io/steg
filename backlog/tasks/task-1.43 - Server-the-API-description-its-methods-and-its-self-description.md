---
id: TASK-1.43
title: 'sdk: the API description, validation against it, and typed results'
status: In Progress
assignee:
  - '@bruk.w2'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T14:15:00Z'
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
doc-8: R36, R37; doc-16: R101, R105, R106. decision-1. The API is one value in the sdk. Split on 2026-11-15 by Bruk from the task that also covered the server's dispatch, which is now a task of its own: the CLI and the sdk's protocol logic need only this half. Decided the same day: `api show` with no target returns the whole description, so a client that cannot import the Rust value gets the entire API in one call.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every method the first release needs (doc-2, section 8) is defined once, in the description, with its parameters and documentation, checked against the list by a test
- [ ] #2 A request is validated against the description with no I/O: an unknown method, an undeclared parameter, a missing required parameter and a value outside a closed set are each refused
- [ ] #3 Each served method's result is a typed shape in the sdk that serialises and deserialises, and the sdk's existing shapes have their wire form there too, with round-trip tests (doc-16 R105)
- [ ] #4 The description's events and errors are defined once, each error with the exit status doc-2 section 6 gives it
- [ ] #5 The sdk's new dependencies are for data and serialisation only, named in boundaries.toml with the reason (doc-16 R106)
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

## Typed results, and the sdk's existing shapes' wire form

Revised from this plan's first draft, which had dispatch build each response as a
`serde_json::Value` by hand and deliberately left the sdk's existing shapes with no
serde at all. That defined the wire form of every result twice — once by hand in the
server, once implicitly in whatever reads it back — with nothing checking the two
agree, and left `returns` as documentation no type enforced. R105 says a shape that
crosses the API is defined once, in the sdk; this revision does that.

**The sdk's existing shapes get real wire forms, in their own files (`ids.rs`,
`colour.rs`, `pane_kind.rs`, `layout.rs` — editing them, not working around them):**
- `SessionId`/`TabId`/`PaneId`: `Serialize` writes the same string `Display` already
  produces; a new `FromStr` (parsing that hex string back, sharing one private helper
  across the three so the parsing logic exists once) backs `Deserialize`. Round-trip
  test per id type.
- `Colour`, `PaneKind`, `Orientation`: `#[derive(Serialize, Deserialize)]` with
  `#[serde(rename_all = "snake_case")]` — their words (`"violet"`, `"terminal"`,
  `"horizontal"`...), not a derived discriminant.
- `Ratio`: a custom `Serialize`/`Deserialize`, not a derive — the wire form is the `f64`
  in `(0, 1)` doc-15 R95 and doc-14 describe ("a ratio between 0 and 1"); the permille
  `u16` stays the in-process representation R95 doesn't mandate. Round-trip test at a
  few values, including one that doesn't divide evenly (333/1000), to catch a rounding
  bug rather than assume one away.
- `Layout`: `#[derive(Serialize, Deserialize)]` over the above (serde's default,
  externally-tagged shape — `{"pane": "<id>"}` / `{"split": {...}}` — not internally
  tagged, which can't represent a newtype variant holding a string). Round-trip test on
  a small tree (one split, two leaves).
- `Direction`/`Size` are left alone: `Direction` already crosses the API as one of
  `ValueKind::Choice`'s plain strings (a parameter value, not a serialized enum), and
  nothing wire-facing holds a `Size` yet.

**One new record type**, `crates/sdk/src/record.rs`: `SessionRecord { id: SessionId,
label: Option<String>, position: usize, colour: Colour, current_tab: TabId }`, serde
derived, field names exactly doc-14's session row. This is the *only* new record type
this task adds — `session.list`/`session.show` are the only served methods whose
`returns` is a record at all; `TabRecord`/`PaneRecord` wait for TASK-1.39/1.41, which is
exactly what "a not-yet method's `returns` is described but its result type isn't
required until its task adds the fields" means in practice.

**Dispatch returns typed values, serialised at the edge**, not built as JSON by hand
per arm. `crates/server/src/dispatch/outcome.rs` (new): an `Outcome` enum — `Nothing`,
`Session(SessionId)`, `Tab(TabId)`, `Pane(PaneId)`, `SessionRecord(SessionRecord)`,
`SessionRecords(Vec<SessionRecord>)`, `Method(Box<Method>)`, `Methods(Vec<Method>)`,
`Description(Box<Description>)` — covers every served method's result with nine cases.
Each dispatch arm returns `Result<Outcome, ApiError>`; one function,
`Outcome::into_value`, is the only place that turns one into the `Value` `Response::Ok`
carries: `Nothing` → `Value::Null`; an id variant → `{"id": "<it>"}` (naming decision-1
section 8's own flagged rough edge — "the real format should name that field" — rather
than leaving it unnamed); the two record variants and `Method`/`Methods`/`Description`
→ `serde_json::to_value` of the already-typed value.

**Tied together with a test** (not just asserted in prose): for every served method
whose `returns` is `Record`/`Records` (`session.show`/`session.list`), the serialized
`SessionRecord`'s own JSON keys are compared against that method's declared `fields`
names — if a field is added to one and not the other, this fails, which is what keeps
`returns` from drifting out from under the type that actually produces it. `api.list`/
`api.show`/`api.call` are checked on their own terms (their result *is* `Method`/
`Description`, so the comparison is "the `Method`/`Description` that comes back
deserializes to the same value that went in," not a `fields`-name comparison).

`Request`/`Response`/`ApiError` (the request side, and the `Response::Ok(Value)`
envelope `Outcome::into_value` feeds) are unchanged from the first draft: that boundary
— a method name and a JSON map of parameters in, one JSON value out — is decision-2's
own "control payloads are JSON," not a shortcut past rust.md's "typed struct at a
boundary," and how it reaches an actual socket is still TASK-1.44/1.45/1.70's, untouched
here.

`events()` seeds doc-8 R38's M1 names now (so `--for <event>` has something to validate
against before TASK-1.49, per doc-2 section 5): `session.created/closed/focused`,
`tab.created/closed/focused`, `pane.created/closed/focused`,
`pane.cwd_changed/title_changed/bell/exited/status_changed` — 14 records, fields drawn
from doc-14's record tables where it has them, as plain `Field` descriptions (not new
sdk result types — nothing dispatches an event in this task). TASK-1.46 fills in
behaviour; the names and fields are fixed from here so nothing downstream guesses at
them twice.

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
- `crates/sdk/src/lib.rs`: `mod description; mod record; pub use description::{...};
  pub use record::SessionRecord;` (mod/pub use only, per rust.md); update the crate doc
  comment.
- `crates/sdk/src/ids.rs`: `Serialize`/`Deserialize`/`FromStr` for `SessionId`,
  `TabId`, `PaneId`, as above.
- `crates/sdk/src/colour.rs`: serde derive on `Colour`.
- `crates/sdk/src/pane_kind.rs`: serde derive on `PaneKind`.
- `crates/sdk/src/layout.rs`: serde derive on `Orientation`; custom serde on `Ratio`;
  serde derive on `Layout`.
- `crates/sdk/src/record.rs` (new): `SessionRecord`.
- `crates/sdk/src/description/mod.rs` (new): `mod`/`pub use` only.
- `crates/sdk/src/description/shapes.rs` (new): `Description`, `Version`, `MethodName`,
  `Method`, `Param`, `Role`, `ValueKind`, `Returns`, `Field`, `Event`, `ErrorRecord`,
  `ErrorCode` (ten variants: `BadRequest`, `NoSuchTarget`, `AmbiguousTarget`,
  `ProgramRunning`, `PositionOutOfRange`, `SplitTooSmall`, `NoSplitInDirection`,
  `PanesNotInSameTab`, `AlreadyInTab`, `NotImplementedYet`), each with serde derives;
  `Description::method/nouns/verbs_of`, `Param::flag/negated_flag`,
  `Method::target/program`.
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
  `pane.unset --zoomed`. Pure data, no event, no I/O.
- `crates/server/Cargo.toml`: add `serde_json`.
- `crates/server/src/lib.rs`: `mod dispatch; pub use dispatch::{dispatch, Context};
  pub use dispatch::outcome::Outcome;`; update the crate doc comment (it currently says
  "behaviour lands starting with TASK-1.9" — this is the first behaviour).
- `crates/server/src/dispatch/mod.rs` (new): `Context`, `dispatch(tree: &mut Tree,
  request: &Request, context: Context) -> Response`, the top-level match by noun,
  `not_implemented_yet(name) -> ApiError`.
- `crates/server/src/dispatch/outcome.rs` (new): `Outcome`, `Outcome::into_value`.
- `crates/server/src/dispatch/session.rs`, `tab.rs`, `pane.rs`, `api.rs` (new): one
  file per noun's served arms, each a plain function over `&mut Tree` returning
  `Result<Outcome, ApiError>`.
- `crates/server/src/dispatch/kernel_error.rs` (new): `map_kernel_error(steg_kernel::Error)
  -> ApiError`, one arm per kernel `Error` variant.
- `crates/server/tests/dispatch.rs` (new, functional — a real `Tree`, no doubles, per
  testing-rust.md): the AC #2/#3 cases below, plus the `Outcome`/`fields` cross-check.
- `spikes/api-description/` (deleted, AC #4): every file under it.

## Order of work

1. `boundaries.toml` + `crates/sdk/Cargo.toml`: add the two dependencies. `just
   check-boundaries` and `just test` (still a no-op crate) confirm the edit alone is
   clean before any behaviour is written.
2. `crates/sdk/src/ids.rs`, `colour.rs`, `pane_kind.rs`, `layout.rs`: the wire forms
   above, each with its own round-trip test, before anything downstream depends on
   them.
3. `crates/sdk/src/record.rs`: `SessionRecord`, with a round-trip test and a test that
   its JSON keys are exactly `{id, label, position, colour, current_tab}`.
4. `crates/sdk/src/description/shapes.rs`: the description's own shape types and their
   derives. Unit test: one example `Method` round-trips through `serde_json`.
5. `crates/sdk/src/description/request.rs`: `Request`/`Response`/`ApiError`/`validate`.
   Port the spike's three validate tests (undeclared parameter, missing required,
   closed-set violation) and add "unknown method → `BadRequest`".
6. `crates/sdk/src/description/methods.rs`: all 38 methods, 14 events, 10 errors,
   assembled by `description()`. Tests: the method set is exactly doc-2 section 8's 38
   `(noun, verb)` pairs, no duplicates; every summary/details/param-summary is
   non-empty; no method declares a parameter named after a CLI-only flag (`json`,
   `quiet`, `verbose`, `no_color`, `no_input`, `help`, `host`, `autostart`); at most one
   `Target` and one `Program` role per method (ported from the spike); every error a
   method names has a record in `errors()`.
7. `crates/sdk/src/lib.rs`: wire the new modules in.
8. `crates/kernel/src/tree.rs`: `set_tab_current_pane` and `tab_of_pane`, with unit
   tests — restoring a tab's current pane does not move the session's `current_tab`
   even when that tab is not the session's current one; `tab_of_pane` finds the right
   tab and returns `None` for an unknown pane.
9. `boundaries.toml` + `crates/server/Cargo.toml`: add `serde_json` to the server.
10. `crates/server/src/dispatch/outcome.rs`: `Outcome` and `into_value`.
11. `crates/server/src/dispatch/`: `Context`, the top-level match, and the four
    per-noun files — 24 real arms, 14 `not_implemented_yet` arms each citing its task in
    a comment, `kernel_error.rs`'s full mapping. `pane.split`/`tab.new`'s `--no-focus`
    path uses step 8's additions; `pane.set/unset --zoomed` uses `fill_pane`/
    `unfill_pane` (`tab_of_pane` for `unfill_pane`'s `TabId`); `*.close`'s `force` and
    `session.new`'s `focus` are accepted and documented no-ops (see above).
12. `crates/server/src/lib.rs`: wire `dispatch`/`Context`/`Outcome` in.
13. `crates/server/tests/dispatch.rs`: the AC #2/#3 cases (listed under Proof), the
    `Outcome`-versus-`fields` cross-check, and two small end-to-end scenarios: split →
    set label → close a session through dispatch alone, and `tab.new` with
    `focus=false` leaving the original tab current.
14. Delete `spikes/api-description/`.
15. `just check` in full; fix what it finds; record the final run.

Steps 1–7 (the sdk's shapes, description, validation and the one new record type, with
no server change) are one reviewable unit that TASK-1.49 (the CLI) could build against
on their own; steps 8–15 (dispatch, the kernel addition, error mapping, `--no-focus`,
deletion) are the second. This revision touches four more existing sdk files than the
first draft (`ids.rs`, `colour.rs`, `pane_kind.rs`, `layout.rs`) and adds the `Outcome`
type and its cross-check test — real growth, not padding, since it is what makes
`returns` an enforced contract rather than documentation. Flagging the split for the
engineer to decide; not resizing the task myself.

## Risks

- **38 methods is a lot of literal data to get right by hand.** Mitigated by step 6's
  completeness test (exact match against a doc-2-derived list) and by grouping into
  per-noun functions, as the spike's own section 8 recommends staying readable to.
- **The served/not-implemented split is this plan's own reading of what "every field
  the shape promises" means**, not something doc-2/doc-14 states as a rule. Mitigated
  by the table above being explicit and reviewable against those documents directly,
  and by every `not_implemented_yet` arm citing its task in code, not just in the plan.
- **`Ratio`'s `f64` round-trip is the one place float imprecision could silently drift**
  a value by one permille. Mitigated by testing an uneven value (333/1000), not only
  round numbers, and by `Deserialize` rounding rather than truncating.
- **`Tree::set_tab_current_pane`/`tab_of_pane` are new kernel surface** outside what
  TASK-1.36–1.38 shipped. Kept deliberately minimal (pure data, no event) so TASK-1.46's
  eventing wraps them rather than replacing them.
- **`api.show` with no target answering the whole description** is a reading of doc-2,
  not an explicit instruction. Flagged above for the engineer; if overruled, only
  `dispatch/api.rs` and its tests change.
- **sdk's first-ever external dependency, and its first serde derives on types already
  on `main`.** `just check` (licence, boundaries, lint) is run after step 1, not only at
  the end, to catch a licence or clippy surprise early — `Unicode-3.0` (reached through
  `serde_derive`'s `syn`) is already on `deny.toml`'s allow list, and `serde`/
  `serde_json` are already in `Cargo.lock` via other dependencies, so no new licence is
  expected, only confirmed; `just check-packages` (kernel built alone) confirms serde
  reaching it transitively through the sdk's now-non-empty dependency list doesn't
  break its own no-I/O lints (serialisation code is not I/O, but checked rather than
  assumed).

## Proof

- AC #1: step 6's tests (exact 38-method set against doc-2 section 8; no empty
  summary/details/param-summary; no CLI-only-flag parameter; at most one target/program
  each); step 2/3's round-trip tests; `just test` for `steg-sdk`.
- AC #2: `dispatch` tests for an unknown method, an undeclared parameter, a missing
  required parameter, and a word outside a closed set, each asserting `ApiError::code
  == BadRequest`; a completeness test that every described method has a dispatch arm
  and every arm is described (both directions, the two lists from the table above); a
  test that every `steg_kernel::Error` variant maps to an `ApiError`; the split/close
  end-to-end scenario; `just test` for `steg-server` and `steg-kernel`.
- AC #3: `api.list` returns all 38 methods once; `api.show <name>` returns one method
  in full (every param, its summary); `api.show` with no target returns the whole
  `Description`; `api.call` dispatches to the named method with its params and matches
  calling it directly; `api.call` of `api.call` is refused; the `Outcome`-versus-
  `fields` cross-check for `session.show`/`session.list`.
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

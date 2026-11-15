---
id: TASK-1.43
title: 'sdk: the API description, validation against it, and typed results'
status: In Progress
assignee:
  - '@bruk.w2'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T15:08:00Z'
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
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '6427b7d1fb39'
asdlc_plan_approved_size: size-4
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
**described now**, with its `returns` shape's `fields` written out whether or not a
kernel operation exists yet to produce them. What this task does **not** do is dispatch
— that is the sibling task now drafted for it. For a method the kernel cannot serve in
full today, the description still says what it returns; no Rust type backs that shape
until the task that gives the kernel the missing fields (programs, status, the terminal
engine, clients, server lifecycle) adds one. Only `session.show`/`session.list` — the
one served pair whose `returns` is a record — get a real sdk type, `SessionRecord`, in
this task. `TabRecord`/`PaneRecord`/anything for `server`/`event`/`client` are
explicitly left to TASK-1.39/1.41/1.44/1.46/1.42, as this split's engineer confirmed.

## The method table, carried for the dispatch task (not built here)

All 38 methods are described in this task regardless of column:

| Noun | Served (kernel can do it today) | Not yet (needs) |
|---|---|---|
| session | list, show, new, close, move, set, unset | focus (needs a client/window to address — TASK-1.42/1.46) |
| tab | new, close, move, set, unset, focus | list, show (title, shown_status — TASK-1.39, 1.41) |
| pane | split, close, move, set, unset, focus, resize, swap | list, show (title, cwd, program, exit_code, status — TASK-1.39–1.41); send, capture, wait (terminal engine — TASK-1.39/1.40) |
| server | — | show, start, stop (TASK-1.44) |
| event | — | watch (TASK-1.46) |
| client | — | list, show (TASK-1.42) |
| api | list, show, call | — |

This table, and everything about dispatch below the line marked **"starting design for
the dispatch task"** in this task's notes, is not this task's scope — reading it is
what a reviewer needs to confirm this plan's `returns` shapes line up with what dispatch
will eventually produce, not something this task builds or tests.

## Where validation lives, and why

Validation (a request's method and parameter names, required-ness, and closed-set
values against the description) is pure data-in data-out over `Description` and
`Request` — no kernel, no I/O, no `Tree`. It lives in **`steg-sdk`** (R101: "the
protocol's logic"), so the same function the dispatch task's server uses for its own
acceptance criteria is also what a future CLI (TASK-1.49) can use for doc-2 section 5's
"the CLI checks the name before any request," with no second implementation. This task
builds and tests `validate` against the description with nothing calling into it beyond
that — there is no `Tree`, no `&mut` anything, nowhere in this crate.

## Typed results, and the sdk's existing shapes' wire form

R105 says a shape that crosses the API is defined once, in the sdk. This task is where
that happens for the shapes it already owns and the one new record `session.show`/
`session.list` need — not by building dispatch to prove it works, but by giving each
shape its wire form and a round-trip test.

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
  `u16` stays the in-process representation R95 doesn't mandate. Round-trip tests at a
  few values, including one that doesn't divide evenly (333/1000); a test that `0.0` and
  `1.0` are refused on the way in, since `Ratio::new` itself refuses them.
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
this task adds. For every "not yet" method whose `returns` is a `Record`/`Records` —
`tab.show`/`list`, `pane.show`/`list`, `client.show`/`list`, `server.show` — the
description still states the shape in full (`Returns::Record{fields}`, each `Field`
named and typed per doc-14), because `fields` is already a complete, self-describing
answer to R37 on its own; no Rust struct backs it until the task that gives the kernel
those fields (TASK-1.39/1.41/1.42/1.44) adds one in the sdk, the same way this task adds
`SessionRecord`. Nothing here fabricates a `TabRecord` with fields the kernel can't fill.

**What this task does not build**: anything that turns a `SessionRecord` (or any other
typed result) into the `Response::Ok(Value)` a client reads back, and anything that
calls a kernel `Tree` to produce one. `Request`, `Response` and `ApiError` are declared
here (R101: they're protocol shapes), but only `ApiError` is exercised by this task's
own tests — `Response::Ok` has nothing to carry until the dispatch task builds the
`Tree`-backed values that fill it. That task's design for doing so (an `Outcome` enum,
one `into_value` edge function, a cross-check test tying each served method's result to
its declared `fields`) is carried in this task's notes under "starting design for the
dispatch task," not duplicated here.

**Tied together with a test anyway, where it costs nothing to**: `SessionRecord`'s own
serialized JSON keys are compared against `session.show`'s and `session.list`'s
declared `fields` names, entirely inside the sdk — no dispatch needed for this one,
since both sides (the type and the description) already live here. This is the one
place AC #3's "serialises and deserialises" and AC #1's "defined... with its
parameters" meet without waiting for the sibling task.

`events()` seeds doc-8 R38's M1 names now (so `--for <event>` has something to validate
against before TASK-1.49, per doc-2 section 5): `session.created/closed/focused`,
`tab.created/closed/focused`, `pane.created/closed/focused`,
`pane.cwd_changed/title_changed/bell/exited/status_changed` — 14 records, fields drawn
from doc-14's record tables where it has them, as plain `Field` descriptions (not new
sdk result types — nothing emits an event anywhere yet). TASK-1.46 fills in behaviour;
the names and fields are fixed from here so nothing downstream guesses at them twice.

`api.show`'s `target` parameter is declared `required: false`; its `returns` is
`Record{fields: method_fields()}`, the shape of one `Method`. "With no target, answer
with the whole `Description` instead" (confirmed by Bruk) is a dispatch-time branch, not
a second `returns` case — `returns` is one shape per method, and the branch changes
which value fills it, not the method's declared shape. Its `details` text says so in
prose; the dispatch task is where the branch is actually built and where the mismatch
between a bare `Method`'s fields and a whole `Description`'s needs a decision about
how `--json` presents it. Carried to that task's notes, not solved here.

## Files that change

- `boundaries.toml`: add `serde` and `serde_json` to `[steg-sdk]`'s `allow`, with a
  reason under R106 (data/serialisation only); rewrite that table's `reason` line,
  which currently says "has nothing of steg's to depend on" and would be false (AC #5).
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
  from doc-2 sections 3, 5, 8 and decision-4), `events()` (14 records), `errors()` (ten
  records with exit statuses per doc-2 section 6: `BadRequest`→2, `NoSuchTarget`→3,
  `AmbiguousTarget`→3, `PositionOutOfRange`→2, `ProgramRunning`→1, `SplitTooSmall`→1,
  `NoSplitInDirection`→1, `PanesNotInSameTab`→1, `AlreadyInTab`→1,
  `NotImplementedYet`→1).

Nothing in `crates/kernel/`, `crates/server/` or `spikes/` changes in this task; those
are the dispatch task's (see its notes).

## Order of work

1. `boundaries.toml` + `crates/sdk/Cargo.toml`: add the two dependencies, with their
   reason. `just check-boundaries` and `just test` (still a no-op crate) confirm the
   edit alone is clean before any behaviour is written.
2. `crates/sdk/src/ids.rs`, `colour.rs`, `pane_kind.rs`, `layout.rs`: the wire forms
   above, each with its own round-trip test, before anything downstream depends on
   them.
3. `crates/sdk/src/record.rs`: `SessionRecord`, with a round-trip test and the
   cross-check against `session.show`/`session.list`'s declared `fields` (written once
   step 6 exists; recorded here as where the type is defined).
4. `crates/sdk/src/description/shapes.rs`: the description's own shape types and their
   derives. Unit test: one example `Method` round-trips through `serde_json`.
5. `crates/sdk/src/description/request.rs`: `Request`/`Response`/`ApiError`/`validate`.
   Port the spike's three validate tests (undeclared parameter, missing required,
   closed-set violation) and add "unknown method → `BadRequest`".
6. `crates/sdk/src/description/methods.rs`: all 38 methods, 14 events, ten errors,
   assembled by `description()`. Tests: the method set is exactly doc-2 section 8's 38
   `(noun, verb)` pairs, no duplicates; every summary/details/param-summary is
   non-empty; no method declares a parameter named after a CLI-only flag (`json`,
   `quiet`, `verbose`, `no_color`, `no_input`, `help`, `host`, `autostart`); at most one
   `Target` and one `Program` role per method (ported from the spike); every error a
   method names has a record in `errors()`; `events()` has R38's 14 M1 names once each;
   `errors()`'s exit statuses match doc-2 section 6. Then go back and finish step 3's
   cross-check test, now that `description()` exists to check against.
7. `crates/sdk/src/lib.rs`: wire the new modules in. `just check` in full; fix what it
   finds; record the final run.

## Risks

- **38 methods is a lot of literal data to get right by hand.** Mitigated by step 6's
  completeness test (exact match against a doc-2-derived list) and by grouping into
  per-noun functions, as the spike's own section 8 recommends staying readable to.
- **`Ratio`'s `f64` round-trip is the one place float imprecision could silently drift**
  a value by one permille. Mitigated by testing an uneven value (333/1000), not only
  round numbers, and by `Deserialize` rounding rather than truncating.
- **A "not yet" method's `returns` is written without a backing type to check it
  against** (no `TabRecord` exists to compare `tab.show`'s `fields` to, the way
  `SessionRecord` is compared for `session.show`). That gap is real and accepted — it's
  exactly what "the result type arrives with the task that builds the behaviour" means
  — but it does mean this task's own tests can't catch a typo in a not-yet method's
  `fields` the way they catch one in `session.show`'s. Noted, not fixed, since fixing it
  would mean building the type early, which this split is deliberately not doing.
- **sdk's first-ever external dependency, and its first serde derives on types already
  on `main`.** `just check` (licence, boundaries, lint) is run after step 1, not only at
  the end, to catch a licence or clippy surprise early — `Unicode-3.0` (reached through
  `serde_derive`'s `syn`) is already on `deny.toml`'s allow list, and `serde`/
  `serde_json` are already in `Cargo.lock` via other dependencies, so no new licence is
  expected, only confirmed.

## Proof (which test proves which clause)

- **AC #1** (every method defined once, checked by a test): step 6's exact-38-pairs
  test; the non-empty summary/details/param-summary test; the no-CLI-only-flag test;
  the at-most-one-target/program test.
- **AC #2** (validated with no I/O): the four `validate` tests (unknown method,
  undeclared parameter, missing required parameter, value outside a closed set), each
  asserting `ApiError::code == BadRequest`; "no I/O" is additionally what `steg-sdk`'s
  own `clippy.toml` mechanically refuses, not only something read by eye — `just check`
  (which runs `just lint`) is the proof that held.
- **AC #3** (typed results serialise/deserialise, existing shapes have wire forms,
  round-trip tests): the id/`Colour`/`PaneKind`/`Orientation`/`Ratio`/`Layout`/
  `SessionRecord` round-trip tests from steps 2–3; the `SessionRecord`-versus-`fields`
  cross-check.
- **AC #4** (events and errors defined once, each error's exit status as doc-2 section
  6 gives it): the 14-events-once test; the errors-exit-status test; the
  every-named-error-has-a-record test.
- **AC #5** (new dependencies for data/serialisation only, named with a reason): the
  `[steg-sdk]` entry in `boundaries.toml` itself, with its reason; `just check-boundaries`
  and `just deny` passing.
- `just check` recorded in full at the end.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan for the narrowed task checked against doc-8 (R36, R37), doc-16 (R101, R105, R106) and decision-1 by @bruk.o: ok. All 38 methods of doc-2 section 8 described once, with 14 events and 10 errors carrying their exit statuses; validation in the sdk with no I/O; serde added to the sdk and named in boundaries.toml; the sdk's identifiers, colour, pane kind, orientation, ratio and layout given their wire form with round-trip tests; one record type, SessionRecord, since sessions are the only records the kernel can fill today, with a test that its fields are the ones the description declares. Records for tabs and panes arrive with the tasks that supply their fields. A ratio travels as a number between 0 and 1 and is held in thousandths, rounded on the way in.

Check passed: `just check` exited 0 at 2026-11-15T14:44:33+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.43 on task-1.43-server-the-api-description-its-methods at 2776d3cd43c5:

```text
ok      format check (0.1s)
ok      lint (87.0s)
ok      boundaries (0.2s)
ok      licences (0.8s)
skip    tests: this change touches no crate
ok      build (steg-sdk) (0.2s)
ok      test (steg-sdk) (0.5s)
ok      build (steg-kernel) (40.0s)
ok      test (steg-kernel) (0.8s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.3s)
ok      build (steg) (0.2s)
ok      test (steg) (0.3s)
ok      build (steg-app) (17.3s)
ok      test (steg-app) (0.7s)
ok      tool tests (1.6s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
```

Check passed: `sh -c 'just check-boundaries && just test'` exited 0 at 2026-11-15T14:56:53+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.43 on task-1.43-server-the-api-description-its-methods at 2776d3cd43c5 with uncommitted changes:

```text
ok      boundaries (0.1s)
ok      tests (every crate) (0.4s)
```

Check passed: `sh -c 'just check-fmt && just lint && just test'` exited 0 at 2026-11-15T15:00:13+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.43 on task-1.43-server-the-api-description-its-methods at 128efc9b70ea with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (0.7s)
ok      tests (every crate) (0.8s)
```

Check passed: `just check` exited 0 at 2026-11-15T15:08:15+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.43 on task-1.43-server-the-api-description-its-methods at 464391efa8f9 with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (0.4s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.1s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (1.3s)
ok      architecture (1.1s)
ok      recipe names (0.0s)
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-15T14:04:00Z

---

Plan written (phase 1). One open flag for the engineer, not decided silently: `api.show` with no target is read in this plan as returning the whole `Description` (decision-1's "served to anyone who asks... at the call"), since doc-2 section 8's row for `api show` doesn't mark its target required and adding a new verb would need a document change. Confirm or overrule.

Also noting for whoever takes this to review/build: AC #4 deletes `spikes/api-description/`, but decision-1 sections 3 and 9 cite `carry-one-method` and `measure.py` under that path as TASK-1.3's recorded evidence — deleting it leaves those references dangling. Not this task's to fix (decision text changes only via a PR containing nothing else); flagging so it isn't lost.

The plan also proposes a possible split at the sdk/server boundary (steps 1-6 vs 7-12) for the engineer to consider, without changing the task's size here.

---

created: 2026-11-15T14:18:00Z

---

## Starting design for the dispatch task

Carried over from this task's first plan (written before the split), for whoever builds the server's dispatch. Not this task's scope; the sdk half's plan no longer includes any of it.

### The method table (doc-2 section 8, 38 rows)

Served now = every field its `returns` shape needs, and every kernel operation it calls, already exists after TASK-1.36/1.37/1.38/1.69.

| Noun | Served now | Not yet (needs) |
|---|---|---|
| session | list, show, new, close, move, set, unset | focus (needs a client/window to address — TASK-1.42/1.46) |
| tab | new, close, move, set, unset, focus | list, show (title, shown_status — TASK-1.39, 1.41) |
| pane | split, close, move, set, unset, focus, resize, swap | list, show (title, cwd, program, exit_code, status — TASK-1.39–1.41); send, capture, wait (terminal engine — TASK-1.39/1.40) |
| server | — | show, start, stop (TASK-1.44) |
| event | — | watch (TASK-1.46) |
| client | — | list, show (TASK-1.42) |
| api | list, show, call | — |

24 served, 14 `not_implemented_yet`. What falls out of "served" that is not full behaviour, to document rather than hide:
- `--force` on `*.close` is accepted and currently a no-op: the kernel has no concept of a running program yet, so nothing it can see is ever running. TASK-1.40 gives it teeth.
- `session.new --focus`/`--no-focus` is accepted and currently a no-op: there is no "current session" in doc-15, only `current_tab`/`current_pane`. What the flag really means (which client's window shows the new session) is R87's, and needs the client model (TASK-1.42) and the session-focus event (TASK-1.46) to do anything.
- `tab.new`/`pane.split --no-focus` DO have real, kernel-level meaning (R115) and should be implemented for real: capture the previous current tab/pane, create, and if `focus=false` put it back, with no event in between (there is no eventing yet, so that half is structural, not separately provable here — TASK-1.46 is where "no focus change announced in between" becomes testable against a subscriber).

### Where dispatch lives, and its signature

Dispatch (actually running an operation) needs `&mut Tree`, which the sdk may never depend on. It goes in `steg-server`, which already depends on both `steg-kernel` and `steg-sdk` (R98). `dispatch` should take an explicit `Context { default_session, default_tab, default_pane }` alongside the `Request`, rather than reading anything off a connection — there is no connection yet (TASK-1.44/1.45), and the kernel's own target resolution (`Tree::resolve_pane_path` etc.) already takes its defaults as parameters, not from tracked state. `dispatch`'s signature should mirror that, so TASK-1.45 ("requests routed to the API") has exactly one thing left to do: build a `Context` from a connection's `STEG_SESSION`/`STEG_TAB`/`STEG_PANE` and pass it through unchanged.

### The kernel addition dispatch needs

`crates/kernel/src/tree.rs`: `Tree::set_tab_current_pane(&mut self, tab: TabId, pane: PaneId) -> Result<(), Error>` (sets only that tab's own current pane, no cascade to the session — unlike `focus_pane`), and `Tree::tab_of_pane(&self, pane: PaneId) -> Option<TabId>` (read-only lookup). Needed for `pane.split --no-focus` (restoring the tab's own current pane without touching the session's `current_tab`) and for `pane.unset --zoomed` (`unfill_pane` takes a `TabId`, and there is currently no public way to find a pane's tab). Pure data, no event, no I/O — keep both minimal so TASK-1.46's eventing wraps them rather than replacing them.

### Outcome: typed results in, one JSON value out

Each dispatch arm should return `Result<Outcome, ApiError>`, not build `serde_json::Value` by hand per arm — that defines the wire form of every result twice (once by hand here, once in the sdk's types) with nothing checking they agree. `crates/server/src/dispatch/outcome.rs`: an `Outcome` enum — `Nothing`, `Session(SessionId)`, `Tab(TabId)`, `Pane(PaneId)`, `SessionRecord(SessionRecord)`, `SessionRecords(Vec<SessionRecord>)`, `Method(Box<Method>)`, `Methods(Vec<Method>)`, `Description(Box<Description>)` — covers every served method's result with nine cases, all built from sdk types that already exist after this task. One function, `Outcome::into_value`, is the only place that turns one into the `Value` `Response::Ok` carries: `Nothing` → `Value::Null`; an id variant → `{"id": "<it>"}` (naming decision-1 section 8's own flagged rough edge — "the real format should name that field" — rather than leaving it unnamed); the record variants and `Method`/`Methods`/`Description` → `serde_json::to_value` of the already-typed value.

Tie it together with a test, not just prose: for every served method whose `returns` is `Record`/`Records`, the serialized typed value's own JSON keys should be compared against that method's declared `fields` names. `api.list`/`api.show`/`api.call` are checked on their own terms (their result *is* `Method`/`Description`, so the comparison is "it round-trips," not a `fields`-name comparison).

`api.show` with no target: Bruk confirmed it returns the whole `Description`. The method's declared `returns` (from the sdk half) is a bare `Method`'s shape; the whole-`Description` case is a dispatch-time branch with a different, richer shape (extra top-level fields: `api_version`, `events`, `errors`, `methods` as an array). Decide in that task how `--json` should present this mismatch (e.g. document it in the method's own `details`, which the sdk half already does) — not solved here.

### Files (in the dispatch task, not this one)

- `crates/kernel/src/tree.rs`: `set_tab_current_pane`, `tab_of_pane`, with unit tests.
- `boundaries.toml` + `crates/server/Cargo.toml`: add `serde_json` to `[steg-server]`'s allow list, reason: building the response payload dispatch answers with.
- `crates/server/src/lib.rs`: `mod dispatch; pub use dispatch::{dispatch, Context}; pub use dispatch::outcome::Outcome;`.
- `crates/server/src/dispatch/mod.rs`, `outcome.rs`, `session.rs`, `tab.rs`, `pane.rs`, `api.rs`, `kernel_error.rs` (new): the top-level match, `Outcome`/`into_value`, one file per noun's served arms, and `map_kernel_error(steg_kernel::Error) -> ApiError` (one arm per kernel `Error` variant).
- `crates/server/tests/dispatch.rs` (functional, a real `Tree`, no doubles per testing-rust.md): per-method dispatch tests, the completeness test (every described method has a dispatch arm and every arm is described, both directions), the kernel-error-mapping test, the `Outcome`-versus-`fields` cross-check, and two end-to-end scenarios (split → set label → close a session; `tab.new` with `focus=false` leaving the original tab current).
- `spikes/api-description/` deleted.

One more thing worth re-flagging when that task is drafted in full: deleting `spikes/api-description/` leaves decision-1 sections 3 and 9's citations of `carry-one-method`/`measure.py` dangling. Decision text changes only via a PR containing nothing else — not any worker's to fix unprompted, but worth a line in that task's own notes too.

---

author: @bruk
created: 2026-11-15T14:41:00Z

---

Plan approved by @bruk (6427b7d1fb39, size-4): its worker may build it.

---
<!-- COMMENTS:END -->

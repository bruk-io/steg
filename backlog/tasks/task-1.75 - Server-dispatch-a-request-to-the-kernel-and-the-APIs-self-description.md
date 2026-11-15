---
id: TASK-1.75
title: 'Server: dispatch a request to the kernel, and the API''s self-description'
status: In Progress
assignee:
  - '@bruk.w2'
created_date: '2026-11-15T14:15:00Z'
updated_date: '2026-11-15T16:01:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.43
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.75-server-dispatch-a-request-to-the-kernel
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-8: R36, R37; doc-16 R98, R101. decision-1. Split from TASK-1.43 on 2026-11-15 by Bruk. Dispatch is a function from a request to a response over the kernel's tree, with no socket: the socket and framing are TASK-1.44, TASK-1.45 and TASK-1.70. The earlier plan on TASK-1.43 has the design to start from: which methods the kernel can serve today and which answer that they are not built yet.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The server validates a request against the description and dispatches it, returning the typed result the sdk defines; an unknown method or parameter is refused
- [ ] #2 Every described method has a dispatch arm and every arm is described, checked both ways by a test; a method whose behaviour is not built yet answers with a distinct error naming what it waits on, never an invented value
- [ ] #3 For every served method, the value returned serialises to exactly the fields the description declares
- [ ] #4 Every kernel error maps to an error a client sees, with the code and exit status the description gives
- [ ] #5 A client can ask for every method and for one method in full, `api show` with no target returns the whole description, and any method can be called by name
- [ ] #6 Creating a tab or a split without taking focus leaves the previous focus in place (doc-15 R115)
- [ ] #7 `spikes/api-description/` is deleted
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Decisions this plan commits to, beyond what TASK-1.43 carried

The carried design (TASK-1.43's notes, comment #2) is the starting point, checked
against what is actually on `main` now (focus exists; a `Terminal` type exists but no
pane holds one — TASK-1.40 is what wires it in). Five things in the carried design do
not survive that check unchanged, and this plan resolves each rather than copying it:

1. **`pane.resize` is reclassified from served to not-yet.** Its declared `by` parameter
   is character cells; `Tree::resize_pane` takes a `Ratio`. Converting cells to a ratio
   needs the tab's current size, and `Tab`'s `size: Option<Size>` has no public getter —
   `set_tab_size` exists but nothing in the real pipeline calls it yet (that arrives with
   TASK-1.42's window-size reporting). Dispatch cannot honestly convert cells to a ratio
   today, so `pane.resize`'s arm answers `NotImplementedYet`, naming the missing size,
   not a fabricated conversion.
2. **`kernel::Error::Engine(ghostty_vt::Error)` has no `ErrorCode` to map to.** It
   arrived with TASK-1.39 after the carried design was written: the sdk has ten codes,
   and none of them means "the engine failed" (`BadRequest` is wrong — not the client's
   fault; `NotImplementedYet` is wrong — it is a real failure, not an absent feature).
   AC #4 asks for an exhaustive match, so this is resolved, not defaulted: this plan adds
   an eleventh `ErrorCode::EngineFailed` to `crates/sdk/src/description/shapes.rs` and
   `errors()`, with exit status 1 (doc-2 section 6: "the command ran and failed" — not
   bad usage, not an ambiguous target, a genuine runtime failure). Flagged here as a
   change to the description TASK-1.43 already landed and reviewed, for the engineer to
   confirm or overrule, not decided silently. In practice no served arm in this task can
   produce it (nothing here calls into the terminal engine yet), but the match must
   still cover it to compile.
3. **`api.call`'s own declared parameters collide with `validate`.** `api.call` declares
   only `target`; `validate` refuses any parameter a method does not declare. A real
   `api.call` request carries the *target method's* parameters alongside `target`
   ("nothing is nested under a second key" — the method's own `details`), so calling
   `validate(&description, &request)` against `api.call`'s own schema first would refuse
   every real call. Dispatch's top-level function special-cases `method == "api.call"`
   **before** the generic validation step: it reads `target` (a string; `BadRequest` if
   absent or not a string), refuses `target == "api.call"` (`BadRequest`: no nested
   calls), builds an inner `Request { method: target, params: request.params minus
   "target" }`, and *that* inner request is what `validate(&description, &inner)` checks
   and what the per-noun match dispatches. `api.call`'s own `errors: [BadRequest]` covers
   the two checks above; the inner call's own errors surface as themselves (so an
   `api.call` of `pane.split` with a bad `direction` answers the same `BadRequest` a
   direct `pane.split` call would, not a different error naming `api.call`).

## The method table, re-derived against today's kernel (not copied from TASK-1.43)

"Served" = every field a method's `returns` needs and every kernel operation it calls
already exists; "not yet" names what it waits on.

| Method | Served? | Waits on |
|---|---|---|
| `session.list`, `.show`, `.new`, `.close`, `.move`, `.set`, `.unset` | Served | — |
| `session.focus` | Not yet | A client/window to address (TASK-1.42, TASK-1.46) |
| `tab.list`, `.show` | Not yet | `title`, `shown_status` (TASK-1.39, TASK-1.41) |
| `tab.new`, `.close`, `.set`, `.unset`, `.focus` | Served | — |
| `tab.move` | Served for `--position`; `--to` (another session) answers `NotImplementedYet` | A kernel operation to move a tab between sessions — `Tree` has none |
| `pane.split`, `.close`, `.move`, `.set`, `.unset`, `.focus`, `.swap` | Served | — |
| `pane.resize` | Not yet (reclassified, see decision 1) | The tab's own size, tracked (TASK-1.42) |
| `pane.list`, `.show` | Not yet | `title`, `cwd`, `program`, `exit_code`, `status` (TASK-1.39–1.41) |
| `pane.send`, `.capture`, `.wait` | Not yet | The terminal engine wired to a pane (TASK-1.39, TASK-1.40) |
| `server.show`, `.start`, `.stop` | Not yet | The server's own socket and lifecycle (TASK-1.44) |
| `event.watch` | Not yet | Events (TASK-1.46) |
| `client.list`, `.show` | Not yet | A client model (TASK-1.42) |
| `api.list`, `.show`, `.call` | Served | — |

25 served (counting `tab.move`'s `--position` branch as served and its `--to` branch as
the one per-call, not whole-method, `NotImplementedYet`), 13 not-yet.

**Accepted-but-not-fully-meaningful, documented rather than hidden** (every one of these
is still a *served* arm — it changes the tree correctly on every field it can act on; it
just also accepts something it cannot yet act on):

- `--force` on `*.close`/`server.stop`: accepted, a no-op (no running-program concept in
  the kernel yet — TASK-1.40 gives it teeth; `ErrorCode::ProgramRunning` is in the
  description but no served arm can produce it today, which is accepted here, not
  treated as a reverse-direction completeness gap — see "Not built both ways" below).
- `session.new --focus`/`--no-focus`: a no-op (no "current session" concept in doc-15,
  only `current_tab`/`current_pane` — R87 is the client/window's, TASK-1.42/1.46).
- `--cwd` and the program after `--` on `session.new`, `tab.new`, `pane.split`: accepted,
  ignored. `Tree::new_session`/`new_tab`/`split_pane` take no working directory and no
  argv; a pane's program is not a field `Pane` has yet (TASK-1.39/1.40). The new pane's
  `kind` is always `PaneKind::Terminal`, since no parameter lets a caller ask for
  anything else.
- `tab.new`/`pane.split --focus`/`--no-focus`: real, not a no-op (R115) — see below.
- `pane.set`/`unset --zoomed`: `set --zoomed`/`set --no-zoomed` fill or unfill for real
  (`Tree::fill_pane`/`unfill_pane`); `unset --zoomed` always unfills, ignoring the
  switch's own boolean value, matching "unset clears a property" regardless of what
  `set`'s boolean carries — the same rule governs `unset --label`, which always clears
  regardless of whether a value was even given.

## `--no-focus` for real (R115; doc-15's own wording: "creating without taking focus is
done by putting the previous focus back")

- **`pane.split --no-focus`**: before calling `Tree::split_pane` (which unconditionally
  makes the new pane current in its tab), capture the tab's *own* current pane
  (`tab.current()`, read via the target's tab). After the split, if `focus == false`,
  call the new `Tree::set_tab_current_pane(tab, previous_pane)` to restore it — this
  touches only that tab's own current pane, never the session's `current_tab`, matching
  `split_pane`'s own scope.
- **`tab.new --no-focus`**: before calling `Tree::new_tab` (which unconditionally makes
  the new tab current in its session, same rule), capture the session's current tab
  (`session.current_tab()`). After, if `focus == false`, call the *existing*
  `Tree::focus_tab(previous_tab)` to restore it. This needs no kernel addition:
  `focus_tab` already sets only `current_tab`, leaving the tab's own current pane alone
  — exactly the restore this case needs. (The carried design proposed a kernel addition
  for this case too; it does not need one, and this plan does not add one it would not
  use.)
- **`session.new --no-focus`**: a no-op, per the table above — there is nothing to
  restore.

### The one kernel addition this plan still needs

`pane.split --no-focus` and `pane.unset --zoomed` (which needs a `TabId` for
`Tree::unfill_pane`, given only a `PaneId` target) are the two real gaps:

- `Tree::tab_of_pane(&self, pane: PaneId) -> Option<TabId>`: a read-only lookup over
  `self.sessions().iter().flat_map(Session::tabs)`, reusing `Session::tabs()` and
  `Tab::pane()`, both already public. No new private helper needed.
- `Tree::set_tab_current_pane(&mut self, tab: TabId, pane: PaneId) -> Result<(), Error>`:
  finds the tab by id (`Error::TabNotFound`) and calls its existing `pub(crate)
  Tab::focus(pane)` (`Error::PaneNotFound` if `pane` is not in that tab) — no cascade to
  the session, unlike `Tree::focus_pane`. Both with unit tests in `tree.rs`'s own test
  module, following its existing style (build a tree, act, assert the one field moved
  and nothing else did).

## Where dispatch lives, and its signature

In `steg-server` (R98): it needs `&mut Tree`, which the sdk may never depend on.
`Context { default_session: SessionId, default_tab: TabId, default_pane: PaneId }`,
built once per request, passed in rather than read off a connection — there is no
connection yet (TASK-1.44/1.45), and the kernel's own `resolve_tab_path`/
`resolve_pane_path` already take their defaults as parameters, not tracked state.

```rust
pub fn dispatch(
    tree: &mut Tree,
    description: &Description,
    context: &Context,
    request: &Request,
) -> Response
```

No socket, no I/O, in this signature — `tree` is already in memory, `request` is already
parsed. TASK-1.45's whole remaining job is building a `Context` from
`STEG_SESSION`/`STEG_TAB`/`STEG_PANE` and calling this unchanged. A target parameter
present in `request.params` is resolved through `Tree::resolve_session`/
`resolve_tab_path`/`resolve_pane_path` against the method's own scope (session/tab/pane);
one left out (the role is `required: false` and the key is absent) uses the matching
`context.default_*` id directly, with no string to resolve at all.

**Position parameters are 1-based on the wire (doc-15 R10/R112) and 0-based in
`Tree::move_session`/`move_tab`** (their tests move to index `0`). Dispatch's `session.move`
and `tab.move --position` arms convert: a wire value of `0` or negative is `BadRequest`
("position must be 1 or greater"), otherwise subtract 1 before calling the kernel. This
is a real conversion, not a passthrough, and gets its own test on each side of the
boundary (`1` → kernel `0`; `0` → `BadRequest`, not a kernel call).

## Typed results: `Outcome` and `into_value`

`crates/server/src/dispatch/outcome.rs`: one enum covering every served method's result,
built only from sdk types that already exist:

```rust
pub enum Outcome {
    Nothing,
    Session(SessionId),
    Tab(TabId),
    Pane(PaneId),
    SessionRecord(SessionRecord),
    SessionRecords(Vec<SessionRecord>),
    Method(Box<Method>),
    Methods(Vec<Method>),
    Description(Box<Description>),
    Text(String),
}
```

`Outcome::into_value(self) -> Value`: `Nothing` → `Value::Null`; an id variant →
`{"id": "<it>"}` (naming decision-1 section 8's flagged rough edge — "the real format
should name that field" — rather than leaving it unnamed); the record/`Method`/
`Methods`/`Description` variants → `serde_json::to_value` of the already-typed value;
`Text` → `Value::String`. `api.list`/`api.show` (with a target) answer `Method`/
`Methods`, the *full* typed struct, matching what the sdk's own
`api_method_fields`/exemption comment already documents (`api.list`'s declared `returns`
is a two-field projection; the actual value is richer, and the sdk's own code already
says this pair is exempt from the fields-cross-check for that reason — not re-decided
here). `api.show` with no target answers `Description` — the whole value, same rule,
same exemption, and its mismatch against a bare `Method`'s declared shape stays
documented in the method's own `details` text as it already is.

## Errors: `map_kernel_error`

`crates/server/src/dispatch/kernel_error.rs`: `fn map_kernel_error(error:
steg_kernel::Error) -> ApiError`, one arm per `Error` variant (13, including `Engine`
once decision 2 above lands), `#[allow]`-free so a new kernel variant fails the build:

| Kernel `Error` | `ApiError` code | Candidates |
|---|---|---|
| `SessionNotFound`/`TabNotFound`/`PaneNotFound` | `NoSuchTarget` | none |
| `TargetNotFound { candidates, .. }` | `NoSuchTarget` | `candidates.iter().map(|c| c.id.clone())` |
| `TargetAmbiguous { candidates, .. }` | `AmbiguousTarget` | same |
| `PositionOutOfRange` | `PositionOutOfRange` | none |
| `InvalidLabel`, `InvalidTargetPath` | `BadRequest` | none |
| `SplitTooSmall` | `SplitTooSmall` | none |
| `NoSplitInDirection` | `NoSplitInDirection` | none |
| `PanesNotInSameTab` | `PanesNotInSameTab` | none |
| `AlreadyInTab` | `AlreadyInTab` | none |
| `Engine` | `EngineFailed` (new) | none |

`ApiError.candidates` is `Vec<String>`; it carries the id alone (`Candidate.id`, already
each candidate's `Display` string), not the label — doc-2 leaves how a client *shows* a
candidate to the CLI, which this task does not build.

**Not built both ways, on purpose**: this match is exhaustive kernel-`Error`-to-code (so
a thirteenth kernel variant fails the build here). The reverse — every `ErrorCode` the
description lists has a kernel source that can produce it today — is not exhaustive and
is not asserted as if it were: `ProgramRunning` has no kernel source yet (no served arm
can trigger it), and `NotImplementedYet` never comes from `map_kernel_error` at all; it
is dispatch's own synthetic error for an arm that is not built. AC #4's "every kernel
error maps" is the forward direction only.

## `api.list`, `api.show`, `api.call`

- `api.list`: `Outcome::Methods(description.methods.clone())`.
- `api.show`: target absent → `Outcome::Description(Box::new(description.clone()))`;
  target present → `description.method(target)` found → `Outcome::Method(Box::new(...))`;
  not found → `ApiError::new(ErrorCode::NoSuchTarget, ...)` (a method name is not an R10
  target, but "named, not found" is still the right code for a client).
- `api.call`: the rewrite in decision 3 above, then dispatch recurses on the inner
  request through the same per-noun match (not a second `dispatch` entry point — one
  function, one match, called once more with the inner `Request`). The outer answer is
  `Outcome::Text(serde_json::to_string(&inner_value)?)` if `api.call`'s declared `Text`
  return is to be taken literally (CLI prints the inner result "as JSON" — doc-2 section
  3); if the inner method's own `Outcome` already converts cleanly to a `Value`, printing
  its `to_string()` is what "as JSON" means at this layer, with the CLI (TASK-1.49) doing
  nothing further.

## Completeness, both ways (AC #2)

Each per-noun file (`session.rs`, `tab.rs`, `pane.rs`, plus `mod.rs`'s own handling of
the always-not-yet nouns `server`, `event`, `client`) matches on `(noun, verb)` as `&str`
tuples — Rust cannot make this exhaustive over arbitrary strings at compile time, so each
file also declares a co-located `const HANDLED: &[(&str, &str)]` naming exactly the pairs
its match handles (the same "two tables, checked by a test" pattern this crate's sibling,
`crate::record`'s `matches_the_fields_...` test, already uses). `mod.rs`'s own test:

- Every one of `description().methods`' 38 dotted names appears in the concatenation of
  every noun's `HANDLED` exactly once (AC #2, forward direction: every described method
  has an arm).
- Every entry in every `HANDLED` names a method that is actually in `description()`
  (AC #2, reverse direction: every arm is described) — catches a typo or a leftover
  arm for a method the sdk no longer declares.
- For every "not yet" entry (the 13-or-14 above, including `pane.resize` and
  `tab.move`'s `--to` branch exercised with a request that sets `to`), calling `dispatch`
  with a minimally valid request for that method returns `Response::Error` with
  `ErrorCode::NotImplementedYet`, and the message names what it waits on (the "Waits on"
  column above, verbatim) — never an invented value (AC #2's second half).

## Typed results versus declared `fields` (AC #3), scoped honestly

The cross-check ("the value returned serialises to exactly the fields the description
declares") applies only where both sides exist and are comparable:

- `session.show`/`session.list`: `SessionRecord`'s keys against `session_fields()` —
  already proven inside the sdk by TASK-1.43's own test; this task's functional test
  re-proves it end to end, through a real `dispatch` call against a real `Tree`, not by
  re-deriving the comparison.
- Every `Nothing`/`Identifier` return (`session.new`/`close`/`move`/`set`/`unset`,
  `tab.new`/`close`/`move`/`set`/`unset`/`focus`, `pane.split`/`close`/`move`/`set`/
  `unset`/`focus`/`swap`): `Identifier` declares no `fields` to cross-check against at
  all (just `of`); the test for these is that `into_value` produces exactly `Value::Null`
  or exactly `{"id": "<the real id>"}` — a shape test, not a fields-name test.
  `api.list`/`.show`/`.call` are excluded from this cross-check entirely, for the reason
  the sdk's own `api_method_fields` comment already gives (the real value is richer than
  the declared projection, by design, already accepted in TASK-1.43's review).

## Files that change

- `crates/kernel/src/tree.rs`: `tab_of_pane`, `set_tab_current_pane`, with unit tests
  (following the existing "before/after, nothing else moved" style in this file's test
  module).
- `crates/sdk/src/description/shapes.rs`: add `ErrorCode::EngineFailed`.
- `crates/sdk/src/description/methods.rs`: add its `ErrorRecord` in `errors()` (exit
  status 1); update the `errors_has_ten_codes_...` test's name and count (now eleven).
- `boundaries.toml`: add `serde_json` to `[steg-server]`'s `allow`, reason: building the
  response payload dispatch answers with (R106's data-only exception, same reasoning as
  the sdk's own entry).
- `crates/server/Cargo.toml`: add `serde_json = "1"`.
- `crates/server/src/lib.rs`: `mod dispatch; pub use dispatch::{dispatch, Context};
  pub use dispatch::outcome::Outcome;` (mod/`pub use` only, per rust.md).
- `crates/server/src/dispatch/mod.rs` (new): `Context`, the top-level `dispatch`
  function (the `api.call` rewrite, then the per-noun match), the completeness consts
  and test.
- `crates/server/src/dispatch/outcome.rs` (new): `Outcome`, `into_value`.
- `crates/server/src/dispatch/session.rs`, `tab.rs`, `pane.rs` (new): each noun's served
  arms and its `HANDLED` const (not-yet arms for that noun included, answering
  `NotImplementedYet`).
- `crates/server/src/dispatch/kernel_error.rs` (new): `map_kernel_error`.
- `crates/server/tests/dispatch.rs` (new, functional — a real `Tree`, no doubles, per
  testing-rust.md): per-method tests for every served arm, the completeness tests both
  ways, the kernel-error-mapping test, the `Outcome`-versus-`fields` cross-check, the
  position-conversion boundary tests, the `api.call` rewrite tests (including refusing
  `api.call` of `api.call`, and calling `pane.split` through `api.call` and getting the
  same answer a direct call would), the `--no-focus` capture-and-restore tests for
  `pane.split` and `tab.new`, and two end-to-end scenarios (split → set label → close a
  session; `tab.new --no-focus` leaving the original tab current).
- `spikes/api-description/` deleted. This leaves decision-1 sections 3 and 9's citations
  of `carry-one-method`/`measure.py` dangling — flagged on this task's notes once more
  (TASK-1.43 raised it twice already) since decision text changes only via a PR
  containing nothing else, not any worker's to fix unprompted here.

## Order of work

1. `crates/kernel/src/tree.rs`: `tab_of_pane`, `set_tab_current_pane`, with unit tests.
   `just test` (kernel only) green before touching the sdk or the server.
2. `crates/sdk/src/description/{shapes,methods}.rs`: `ErrorCode::EngineFailed` and its
   `ErrorRecord`; fix the count-based test it breaks. `just test` (sdk only) green.
3. `boundaries.toml` + `crates/server/Cargo.toml`: the one new dependency, with its
   reason. `just check-boundaries`.
4. `crates/server/src/dispatch/kernel_error.rs` and `outcome.rs`: the two exhaustive,
   data-only pieces, each with its own unit tests, before anything calls into a `Tree`.
5. `crates/server/src/dispatch/{session,tab,pane}.rs` and `mod.rs`'s top-level
   `dispatch`/`Context`/completeness consts: the served arms, the not-yet arms, the
   `api.call` rewrite, the position conversion. Build incrementally noun by noun
   (session, then tab, then pane — tab and pane's `--no-focus` cases depend on each
   other's kernel calls being in place first).
6. `crates/server/tests/dispatch.rs`: every test named in "Files that change" above.
   `just test` (server) green.
7. `spikes/api-description/` deleted. `just check` in full; fix what it finds; record
   the final run.

## Risks

- **This plan is large for one review.** It touches a kernel addition, an sdk error
  addition, and the whole of dispatch. If the engineer agrees, a natural split is **(A)**
  `Context`, `Outcome`/`into_value`, `map_kernel_error`, the completeness tests both
  ways with every arm answering `NotImplementedYet`, `api.list`/`.show`/`.call`, and
  spike deletion — provable with no kernel change beyond the two `Tree` additions;
  **(B)** the kernel-backed session/tab/pane served arms, the `SessionRecord` cross-check
  through a real `dispatch` call, and the `--no-focus` capture-and-restore behaviour.
  Noted for the engineer's decision; this plan is written for the task as assigned and
  does not resize it.
- **The position conversion (1-based wire, 0-based kernel) is a one-line bug waiting to
  happen.** Mitigated by a dedicated boundary test on each side, not folded into a
  general "move works" test.
- **`EngineFailed`'s exit status (1) is a judgement call**, not written down anywhere
  doc-2 section 6 enumerates by name. Flagged for the engineer rather than asserted as
  settled; if overruled, only `errors()`'s one `ErrorRecord` and this plan's own table
  change.
- **The per-noun `HANDLED` consts duplicate the match arms' method names once.** Mitigated
  by colocating each const directly above its match in the same file, so a reviewer sees
  both at once, and by the completeness test itself, which is exactly what catches drift
  between them.

## Proof (which test proves which clause)

- **AC #1** (validated, dispatched, typed result, unknown method/parameter refused): the
  per-method functional tests in `dispatch.rs` for every served arm; `validate`'s own
  sdk-side tests (already proven, re-exercised here through a real `dispatch` call for
  an unknown method and an undeclared parameter).
- **AC #2** (every described method has an arm and every arm is described, both ways by
  a test; a not-built method's own distinct error naming what it waits on): the
  completeness tests described above, both directions; the not-yet-arm tests checking
  `ErrorCode::NotImplementedYet` and the message text for each of the 13-or-14 not-yet
  methods.
- **AC #3** (every served method's result serialises to exactly its declared fields):
  the `SessionRecord`-versus-`session.show`/`session.list` cross-check through a real
  `dispatch` call; the `Identifier`/`Nothing` shape tests for every other served method;
  the stated exemption for `api.list`/`.show`/`.call`.
- **AC #4** (every kernel error maps, with the code and exit status the description
  gives): `map_kernel_error`'s own exhaustive-match unit tests, one per kernel `Error`
  variant; the exit-status lookup is the sdk's own `errors()` table, unchanged except for
  the new `EngineFailed` entry, which gets the same test shape the other ten already
  have.
- **AC #5** (every method callable by name, `api show` with no target returns the whole
  description, `api list`/`api show`/`api call` all work): the `api.*` tests in
  `dispatch.rs`, including the no-target case and the `api.call`-of-`pane.split`
  round trip.
- **AC #6** (creating without focus leaves the previous focus in place, R115): the
  `pane.split --no-focus` and `tab.new --no-focus` capture-and-restore tests, each
  asserting the *whole* tree before and after (following `tree.rs`'s own existing style
  for this kind of assertion), not just the one field expected to move.
- **AC #7** (`spikes/api-description/` deleted): `git status`/the diff itself; noted
  above that this dangles two citations in decision-1, which is not this task's to fix.
- `just check` recorded in full at the end, and after every rebase onto `origin/main`.
<!-- SECTION:PLAN:END -->

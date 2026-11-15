---
id: TASK-1.75
title: 'Server: dispatch a request to the kernel, and the API''s self-description'
status: In Progress
assignee:
  - '@bruk.w2'
created_date: '2026-11-15T14:15:00Z'
updated_date: '2026-11-15T16:32:00Z'
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
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'f570689623ad'
asdlc_plan_approved_size: size-3
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
pane holds one — TASK-1.40 is what wires it in). Four things in the carried design do
not survive that check unchanged, and this plan resolves each rather than copying it:

1. **`pane.resize` is reclassified from served to not-yet.** Its declared `by` parameter
   is character cells; `Tree::resize_pane` takes a `Ratio`. Converting cells to a ratio
   needs the tab's current size, and `Tab`'s `size: Option<Size>` has no public getter
   on `main` today — `set_tab_size` exists but nothing reads it back. TASK-1.42 (planned
   in parallel) adds `Tab::size()`; until it lands, dispatch cannot honestly convert
   cells to a ratio, so `pane.resize`'s arm answers `NotImplementedYet`, naming
   `Tab::size()`/TASK-1.42, not a fabricated conversion.
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
3. **`api.call`'s own declared parameters collide with `validate`, and this plan picks
   one shape for its answer, not two.** `api.call` declares only `target`; `validate`
   refuses any parameter a method does not declare. A real `api.call` request carries
   the *target method's* parameters alongside `target` ("nothing is nested under a
   second key" — the method's own `details`), so calling
   `validate(&description, &request)` against `api.call`'s own schema first would refuse
   every real call. Dispatch's top-level function special-cases `method == "api.call"`
   **before** the generic validation step: it reads `target` (a string; `BadRequest` if
   absent or not a string), refuses `target == "api.call"` (`BadRequest`: no nested
   calls), builds an inner `Request { method: target, params: request.params minus
   "target" }`, validates *that* (`validate(&description, &inner)`), and then — this is
   the one shape, not a choice left open — **calls `dispatch` again on the inner request
   and returns that `Response` unchanged as the outer answer.** No `Outcome` variant
   wraps or re-encodes it: the client reading `api.call`'s answer sees exactly the value
   the inner method's own call would have produced, not a JSON string of it. `returns:
   Text` on `api.call`'s own description describes what the CLI prints (doc-2 section
   3: "the method's result, as JSON" — a rendering rule for TASK-1.49, not a wire
   transform), and this plan does not change that description entry. `Outcome` gets no
   `Text(String)` variant in this task: nothing served here produces one (`pane.capture`,
   the one method whose `returns` is `Text`, is not-yet) — a variant with no producer
   would say something untrue (rust.md), and a later task adds it when `pane.capture`
   becomes servable.
4. **`--cwd` and the program after `--` on `session.new`/`tab.new`/`pane.split` are
   refused, not silently ignored, when given.** The earlier draft of this plan accepted
   both and dropped them, so a caller asking for `pane split -- htop` would get a
   success and a pane's identifier back with no `htop` running in it — a false claim
   (the engineer's correction). The test applied throughout this plan, to this case and
   every other "accepted but not fully meaningful" one below: *does the method's own
   `returns` or documented effect imply the flag's effect took hold?* Where yes, and the
   kernel cannot honour it, the call is refused with `NotImplementedYet`, not answered
   with a success that quietly didn't happen. So: when `cwd` is present, or `program` is
   present (either, or both), the arm does not call `Tree::new_session`/`new_tab`/
   `split_pane` at all — it returns `ApiError::new(ErrorCode::NotImplementedYet, ...)`
   naming a pane's program and working directory, which TASK-1.40 adds
   (`Tree::start_program`, `record_exit`, `restart_program`, and a program/cwd on
   `Pane`), and creates nothing. When neither is given, the method proceeds exactly as
   before — a plain shell pane, which the kernel already fully supports and which is the
   one case `session.new`'s/`tab.new`'s/`pane.split`'s success already promises nothing
   more than ("the new thing's identifier", not "and it runs what you asked"). This
   moves these three methods from "served in full" to "served for the plain-shell case,
   not-yet when a program or cwd is asked for" — the same shape `tab.move` already has
   for `--to`, not a new kind of case.

## Re-applying that same test to the rest of the "accepted but not fully meaningful" list

- **`--force` on `*.close`/`server.stop`: stays accepted, no refusal.** Nothing in the
  kernel can represent a running program yet (TASK-1.40), so there is no state for
  `--force` to guard that could ever differ from "there was nothing to force past" — a
  caller cannot notice a difference between giving it and not, because the thing it
  would override cannot exist. This is the same shape as the engineer's own example.
- **`session.new --focus`/`--no-focus`: stays accepted, no refusal.** `session.new`'s
  `returns` is only the new session's identifier; nothing in it, or in doc-15, claims
  anything about which window shows the session (R87 is the app's, not the kernel's —
  there is no "current session" concept for a kernel-only dispatch to honour or fail to
  honour). A caller cannot observe a difference at this layer between `--focus` and
  `--no-focus` today, because there is no window to compare against, unlike `--cwd`/
  `program`, which the pane's own later `pane.show`/`pane.capture` would show as missing
  the moment those methods are served. `tab.new`/`pane.split --focus`/`--no-focus` are
  the opposite case — see R115 below: those *are* real today, not accepted-and-ignored.
  `pane.set`/`unset --zoomed` are likewise real today (`Tree::fill_pane`/`unfill_pane`),
  not on this list at all.

## The method table, re-derived against today's kernel (not copied from TASK-1.43)

"Served" = every field a method's `returns` needs and every kernel operation it calls
already exists; "not yet" names what it waits on, by the specific type or function that
arrives with that task where it is known (TASK-1.42 adds `Tab::size()`; TASK-1.40 adds
`Tree::start_program`, `record_exit`, `restart_program`, and a program/status on `Pane`
— neither is on `main` yet, so both stay not-yet here and are named precisely so the
task that lands them can find exactly what to flip). Counted directly against
`description().methods`' 38 entries: **19 served in full, 4 served for one branch of
their parameters and not-yet for another (`session.new`, `tab.new`, `pane.split`,
`tab.move`), 15 not served at all. 19 + 4 + 15 = 38.**

| Method | Served? | Waits on |
|---|---|---|
| `session.list`, `.show`, `.close`, `.move`, `.set`, `.unset` (6) | Served | — |
| `session.new` (1) | Served when neither `cwd` nor a program is given; `NotImplementedYet` when either is | A pane's program and working directory (TASK-1.40) |
| `session.focus` (1) | Not yet | A client/window to address (TASK-1.42, TASK-1.46) |
| `tab.list`, `.show` (2) | Not yet | `title`, `shown_status` (TASK-1.39, 1.41) |
| `tab.new` (1) | Served when neither `cwd` nor a program is given; `NotImplementedYet` when either is | Same as `session.new` (TASK-1.40) |
| `tab.close`, `.set`, `.unset`, `.focus` (4) | Served | — |
| `tab.move` (1) | Served for `--position`; its `--to` branch (moving the tab to another session) answers `NotImplementedYet` | A kernel operation to move a tab between sessions — `Tree` has none |
| `pane.split` (1) | Served when neither `cwd` nor a program is given; `NotImplementedYet` when either is | Same as `session.new` (TASK-1.40) |
| `pane.close`, `.move`, `.set`, `.unset`, `.focus`, `.swap` (6) | Served | — |
| `pane.resize` (1) | Not yet | `Tab::size()` (TASK-1.42) |
| `pane.list`, `.show` (2) | Not yet | `title`/`cwd`/`program`/`exit_code`/`status` (TASK-1.39–1.41) |
| `pane.send`, `.capture`, `.wait` (3) | Not yet | `Tree::start_program`/`record_exit`/`restart_program` and a program on `Pane` (TASK-1.40) |
| `server.show`, `.start`, `.stop` (3) | Not yet | The server's own socket and lifecycle (TASK-1.44) |
| `event.watch` (1) | Not yet | Events (TASK-1.46) |
| `client.list`, `.show` (2) | Not yet | A client model (TASK-1.42) |
| `api.list`, `.show`, `.call` (3) | Served | — |

**19 fully served + 4 partial = 23 methods with a real dispatch arm. 15 methods are
wholly not-yet; of the 23 with an arm, 4 also have a per-call `NotImplementedYet`
branch (`session.new`/`tab.new`/`pane.split` given `cwd` or a program; `tab.move --to`)
— 19 total scenarios across this task that answer `NotImplementedYet`**, the number the
completeness test (below) asserts directly against this table, not against a count
restated in prose.

**What else is accepted but not fully meaningful, applying the same test** (does the
method's own `returns`/documented effect imply the flag's effect took hold; if yes and
the kernel cannot honour it, refuse rather than silently succeed):

- `--force` on `*.close`/`server.stop`: accepted, no refusal — see above.
- `session.new --focus`/`--no-focus`: accepted, no refusal — see above.
- The new pane's `kind` is always `PaneKind::Terminal`; there is no `kind` parameter in
  the description for a caller to ask for anything else, so there is nothing to refuse.

## `--no-focus` for real (R115; doc-15's own wording: "creating without taking focus is
done by putting the previous focus back")

- **`pane.split --no-focus`**: before calling `Tree::split_pane` (which unconditionally
  makes the new pane current in its tab), capture the tab's *own* current pane
  (`tab.current()`, read via the target's tab). After the split, if `focus == false`,
  call the new `Tree::set_tab_current_pane(tab, previous_pane)` to restore it — this
  touches only that tab's own current pane, never the session's `current_tab`, matching
  `split_pane`'s own scope. (This runs only on the plain-shell branch — see decision 4
  above; the `cwd`/program branch refuses before any of this.)
- **`tab.new --no-focus`**: before calling `Tree::new_tab` (which unconditionally makes
  the new tab current in its session, same rule), capture the session's current tab
  (`session.current_tab()`). After, if `focus == false`, call the *existing*
  `Tree::focus_tab(previous_tab)` to restore it. This needs no kernel addition:
  `focus_tab` already sets only `current_tab`, leaving the tab's own current pane alone
  — exactly the restore this case needs. (The carried design proposed a kernel addition
  for this case too; it does not need one, and this plan does not add one it would not
  use.) Same plain-shell-branch-only scope as above.
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

Both additions are deliberately event-free, carrying no eventing of their own — exactly
so TASK-1.46 can *wrap* them (fire `pane.focused` around `set_tab_current_pane`, and
whatever event a zoom change gets around `tab_of_pane`'s callers) rather than replace
them when events arrive.

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
parsed. **What TASK-1.45 needs**: exactly this function and `Context`, unchanged — its
whole remaining job is building a `Context` from `STEG_SESSION`/`STEG_TAB`/`STEG_PANE`
and calling `dispatch` once per request it reads off a connection. **What TASK-1.46
needs**: `Outcome` and the per-noun `HANDLED` consts as the seam it attaches events to —
it does not change dispatch's signature or its match; firing `event.watch`'s subscribers
is a wrapper around the same calls, and the two new `Tree` methods above are shaped for
exactly that wrapping. **How a not-built arm or branch becomes real** (for
TASK-1.39/1.40/1.41/1.42 and TASK-1.44/1.46, each of which upgrades some of the 19
not-yet cases above — concretely, TASK-1.40 landing `Tree::start_program`/
`record_exit`/`restart_program` and a program/cwd on `Pane` is what turns
`session.new`/`tab.new`/`pane.split`'s `cwd`/program branch, and `pane.send`/`.capture`/
`.wait`, from `NotImplementedYet` into real arms; TASK-1.42 landing `Tab::size()` is what
turns `pane.resize`'s): the method's entry in its noun's `HANDLED` const does not move —
it is already there, since the method is already described and already matched. Only
the arm's *body* changes, from the shared `not_implemented_yet(name, waiting_on)` call
(or, for the four partial methods, one branch of the arm) to real logic against the
kernel fields that task adds. The only test that is deleted is that one method's (or
branch's) "answers `NotImplementedYet`" case; the completeness tests themselves (which
assert against `description().methods` and each `HANDLED` const, not against a
served/not-yet count) keep passing across the swap unchanged.

A target parameter present in `request.params` is resolved through `Tree::resolve_session`/
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
built only from sdk types that already exist and that this task actually produces (no
variant with no producer, per the `api.call` decision above):

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
}
```

`Outcome::into_value(self) -> Value`: `Nothing` → `Value::Null`; an id variant →
`{"id": "<it>"}` (naming decision-1 section 8's flagged rough edge — "the real format
should name that field" — rather than leaving it unnamed); the record/`Method`/
`Methods`/`Description` variants → `serde_json::to_value` of the already-typed value.
`api.list`/`api.show` (with a target) answer `Method`/`Methods`, the *full* typed
struct, matching what the sdk's own `api_method_fields`/exemption comment already
documents (`api.list`'s declared `returns` is a two-field projection; the actual value
is richer, and the sdk's own code already says this pair is exempt from the
fields-cross-check for that reason — not re-decided here). `api.show` with no target
answers `Description` — the whole value, same rule, same exemption, and its mismatch
against a bare `Method`'s declared shape stays documented in the method's own `details`
text as it already is. `api.call` never calls `into_value` itself — see the decision
above: it returns the inner `dispatch` call's `Response` unchanged.

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
is dispatch's own synthetic error for an arm or branch that is not built. AC #4's "every
kernel error maps" is the forward direction only.

## `api.list`, `api.show`, `api.call`

- `api.list`: `Outcome::Methods(description.methods.clone())`.
- `api.show`: target absent → `Outcome::Description(Box::new(description.clone()))`;
  target present → `description.method(target)` found → `Outcome::Method(Box::new(...))`;
  not found → `ApiError::new(ErrorCode::NoSuchTarget, ...)` (a method name is not an R10
  target, but "named, not found" is still the right code for a client).
- `api.call`: the rewrite-and-recurse in decision 3 above. One function, one match,
  called a second time on the inner `Request`; the outer `Response` is the inner call's
  `Response`, unchanged.

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
- For each of the 15 whole-method not-yet cases, plus the 4 per-call branches
  (`tab.move --to`; `session.new`/`tab.new`/`pane.split` given `cwd` or a program) — 19
  scenarios total, matching the method table above exactly: calling `dispatch` with the
  triggering request returns `Response::Error` with `ErrorCode::NotImplementedYet`, and
  the message names what it waits on (the "Waits on" column above, verbatim) — never an
  invented value, and nothing is created (AC #2's second half; for the four partial
  methods, also checked by asserting the tree is unchanged after the refusal).

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
  function (the `api.call` rewrite-and-recurse, then the per-noun match), the
  completeness consts and test.
- `crates/server/src/dispatch/outcome.rs` (new): `Outcome`, `into_value`.
- `crates/server/src/dispatch/session.rs`, `tab.rs`, `pane.rs` (new): each noun's served
  arms (including the `cwd`/program refusal branch on `session.new`/`tab.new`/
  `pane.split`, and the `--to` refusal branch on `tab.move`) and its `HANDLED` const
  (not-yet arms for that noun included, answering `NotImplementedYet`).
- `crates/server/src/dispatch/kernel_error.rs` (new): `map_kernel_error`.
- `crates/server/tests/dispatch.rs` (new, functional — a real `Tree`, no doubles, per
  testing-rust.md): per-method tests for every served arm, the completeness tests both
  ways, the kernel-error-mapping test, the `Outcome`-versus-`fields` cross-check, the
  position-conversion boundary tests, the `api.call` rewrite-and-recurse tests
  (including refusing `api.call` of `api.call`, and calling `pane.split` through
  `api.call` and getting byte-for-byte the same `Response` a direct call would), the
  `--no-focus` capture-and-restore tests for `pane.split` and `tab.new`, the four
  cwd/program/`--to` refusal tests asserting both the error and that nothing was
  created, and two end-to-end scenarios (split → set label → close a session; `tab.new
  --no-focus` leaving the original tab current).
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
   `dispatch`/`Context`/completeness consts: the served arms (including each partial
   method's refusal branch, checked first so nothing is created before the rest of the
   arm is written), the not-yet arms, the `api.call` rewrite-and-recurse, the position
   conversion. Build incrementally noun by noun (session, then tab, then pane — tab and
   pane's `--no-focus` cases depend on each other's kernel calls being in place first).
6. `crates/server/tests/dispatch.rs`: every test named in "Files that change" above.
   `just test` (server) green.
7. `spikes/api-description/` deleted. `just check` in full; fix what it finds; record
   the final run.

## Risks

- **This plan is large for one review — flagged explicitly, not only here.** The
  engineer has already indicated it should stay one change, since neither of the two
  halves this plan's earlier draft proposed splitting on (dispatch infrastructure versus
  kernel-backed arms) unblocks anything without the other. Kept as one task, as
  instructed; not resized here.
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
- **A refusal branch that runs after some kernel mutation would be a worse bug than the
  one it replaces** (half-created state reported as `NotImplementedYet`). Mitigated by
  checking `cwd`/program presence before any `Tree` call on the three affected methods,
  and by the "tree unchanged after the refusal" assertion named above.

## Proof (which test proves which clause)

- **AC #1** (validated, dispatched, typed result, unknown method/parameter refused): the
  per-method functional tests in `dispatch.rs` for every served arm; `validate`'s own
  sdk-side tests (already proven, re-exercised here through a real `dispatch` call for
  an unknown method and an undeclared parameter).
- **AC #2** (every described method has an arm and every arm is described, both ways by
  a test; a not-built method's own distinct error naming what it waits on): the
  completeness tests described above, both directions; the 19 not-yet-scenario tests
  checking `ErrorCode::NotImplementedYet`, the message text, and that nothing was
  created, matching the method table's count exactly.
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Revised plan checked against doc-8 (R36, R37), doc-2, doc-15 (R115), doc-16 and decision-1 by @bruk.o: ok. Dispatch is a function from a request and a context to a response over the tree; every one of the 38 methods has an arm, real or answering not built with what it waits on, checked both ways; a working directory or a program given to session new, tab new or pane split is refused until TASK-1.40 lands, and nothing a caller would notice missing is accepted silently; kernel errors are matched exhaustively. For the engineer: a new error code, EngineFailed, with exit status 1, and pane resize and tab move --to answer not built for now. Kept as one change.

Check passed: `just check` exited 0 at 2026-11-15T16:31:19+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.75 on task-1.75-server-dispatch-a-request-to-the-kernel at c8dc7cdd8d3d:

```text
ok      format check (0.1s)
ok      lint (99.4s)
ok      boundaries (0.2s)
ok      licences (0.8s)
skip    tests: this change touches no crate
ok      build (steg-sdk) (3.1s)
ok      test (steg-sdk) (1.2s)
ok      build (steg-kernel) (51.4s)
ok      test (steg-kernel) (1.0s)
ok      build (steg-server) (0.2s)
ok      test (steg-server) (0.3s)
ok      build (steg) (0.2s)
ok      test (steg) (0.3s)
ok      build (steg-app) (17.9s)
ok      test (steg-app) (0.7s)
ok      tool tests (1.4s)
ok      architecture (1.0s)
ok      recipe names (0.1s)
```

Check passed: `just test` exited 0 at 2026-11-15T16:32:05+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.75 on task-1.75-server-dispatch-a-request-to-the-kernel at c8dc7cdd8d3d with uncommitted changes:

```text
ok      tests (-p steg -p steg-kernel -p steg-server) (1.0s)
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-15T16:26:00Z

---

Plan approved by @bruk (f570689623ad, size-3): its worker may build it.

---
<!-- COMMENTS:END -->

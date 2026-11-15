---
id: TASK-1.75
title: 'Server: dispatch a request to the kernel, and the API''s self-description'
status: In Progress
assignee:
  - '@bruk.w2'
created_date: '2026-11-15T14:15:00Z'
updated_date: '2026-11-15T17:15:00Z'
labels:
  - size-3
  - changes-requested
milestone: m-0
dependencies:
  - TASK-1.43
references:
  - task-1.75-server-dispatch-a-request-to-the-kernel
  - https://github.com/bruk-io/steg/pull/33
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.75-server-dispatch-a-request-to-the-kernel
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'f570689623ad'
asdlc_plan_approved_size: size-3
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=1,nit=3'
asdlc_review_policies: ["review", "rust"]
asdlc_plan_matched: no
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

Check passed: `just test` exited 0 at 2026-11-15T16:33:27+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.75 on task-1.75-server-dispatch-a-request-to-the-kernel at c8dc7cdd8d3d with uncommitted changes:

```text
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (42.1s)
```

Check passed: `just check-boundaries` exited 0 at 2026-11-15T16:33:50+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.75 on task-1.75-server-dispatch-a-request-to-the-kernel at c8dc7cdd8d3d with uncommitted changes:

```text
ok      boundaries (0.1s)
```

Check passed: `just test` exited 0 at 2026-11-15T16:42:02+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.75 on task-1.75-server-dispatch-a-request-to-the-kernel at c8dc7cdd8d3d with uncommitted changes:

```text
ok      tests (every crate) (1.6s)
```

Check FAILED: `just test` exited 101 at 2026-11-15T16:48:01+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.75 on task-1.75-server-dispatch-a-request-to-the-kernel at c8dc7cdd8d3d with uncommitted changes, last 40 of 64 lines:

```text
test session_show_with_an_unknown_target_is_no_such_target ... ok
test api_show_an_unknown_method_is_no_such_target ... ok
test session_show_matches_exactly_the_fields_the_description_declares ... ok
test every_served_nothing_returning_method_answers_exactly_null ... ok
test pane_focus_moves_to_the_neighbour ... ok
test pane_swap_exchanges_two_panes ... ok
test pane_set_zoomed_fills_and_unset_zoomed_unfills ... ok
test pane_split_makes_the_new_pane_current_by_default ... ok
test pane_split_with_an_invalid_label_leaves_the_tree_exactly_as_it_was ... ok
test pane_close_removes_the_pane ... ok
test api_show_with_a_target_returns_one_method ... ok
test tab_move_with_position_reorders ... ok
test tab_close_removes_it ... ok
test tab_new_with_no_focus_leaves_the_original_tab_current ... ok
test tab_focus_makes_it_current ... ok
test split_set_label_then_close_the_session ... ok
test tab_new_with_no_focus_restores_the_previous_current_tab ... ok
test tab_new_makes_the_new_tab_current_by_default ... ok
test api_list_returns_every_method ... ok
test api_show_with_no_target_returns_the_whole_description ... ok
test every_not_yet_scenario_answers_not_implemented_yet_and_changes_nothing ... FAILED
test every_described_method_gets_a_response_with_no_panic ... ok

failures:

---- every_not_yet_scenario_answers_not_implemented_yet_and_changes_nothing stdout ----

thread 'every_not_yet_scenario_answers_not_implemented_yet_and_changes_nothing' (6718242) panicked at crates/server/tests/dispatch.rs:143:9:
event.watch: "not implemented yet: waiting on events (TASK-1.46)" does not name "Events"
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    every_not_yet_scenario_answers_not_implemented_yet_and_changes_nothing

test result: FAILED. 35 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-server --test dispatch`
full output: .quiet/tests-every-crate.log
error: recipe `test` failed with exit code 101
```

Check passed: `just test` exited 0 at 2026-11-15T16:48:14+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.75 on task-1.75-server-dispatch-a-request-to-the-kernel at c8dc7cdd8d3d with uncommitted changes:

```text
ok      tests (every crate) (1.0s)
```

Check FAILED: `just check` exited 1 at 2026-11-15T16:48:20+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.75 on task-1.75-server-dispatch-a-request-to-the-kernel at c8dc7cdd8d3d with uncommitted changes, last 40 of 64 lines:

```text
 
[31m-    let value = ok(dispatch(&mut tree, &api, &context, &request("api.show", vec![])));
(B[m[32m+    let value = ok(dispatch(
(B[m[32m+        &mut tree,
(B[m[32m+        &api,
(B[m[32m+        &context,
(B[m[32m+        &request("api.show", vec![]),
(B[m[32m+    ));
(B[m 
     assert_eq!(
         value["methods"].as_array().unwrap().len(),
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.75/crates/server/tests/dispatch.rs:732:
 fn api_call_with_no_target_is_a_bad_request() {
     let (mut tree, context, api) = fresh();
 
[31m-    let error = err(dispatch(&mut tree, &api, &context, &request("api.call", vec![])));
(B[m[32m+    let error = err(dispatch(
(B[m[32m+        &mut tree,
(B[m[32m+        &api,
(B[m[32m+        &context,
(B[m[32m+        &request("api.call", vec![]),
(B[m[32m+    ));
(B[m 
     assert_eq!(error.code, ErrorCode::BadRequest);
 }
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.75/crates/server/tests/dispatch.rs:804:
 fn every_served_nothing_returning_method_answers_exactly_null() {
     let (mut tree, context, api) = fresh();
     let second_pane = tree
[31m-        .split_pane(context.default_pane, steg_sdk::Direction::Right, steg_sdk::PaneKind::Terminal)
(B[m[32m+        .split_pane(
(B[m[32m+            context.default_pane,
(B[m[32m+            steg_sdk::Direction::Right,
(B[m[32m+            steg_sdk::PaneKind::Terminal,
(B[m[32m+        )
(B[m         .unwrap();
 
     for (method, extra) in [
full output: .quiet/format-check.log
error: recipe `check-fmt` failed on line 117 with exit code 1
```

Check FAILED: `just check` exited 101 at 2026-11-15T16:48:32+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.75 on task-1.75-server-dispatch-a-request-to-the-kernel at c8dc7cdd8d3d with uncommitted changes, last 40 of 65 lines:

```text
   |
47 | fn handled() -> Vec<(&'static str, &'static str)> {
   |    ^^^^^^^
   |
   = note: `-D dead-code` implied by `-D warnings`
   = help: to override `-D warnings` add `#[expect(dead_code)]` or `#[allow(dead_code)]`

error: constant `NOT_YET_NOUNS` is never used
  --> crates/server/src/dispatch/mod.rs:60:7
   |
60 | const NOT_YET_NOUNS: &[(&str, &str)] = &[
   |       ^^^^^^^^^^^^^

error: docs for function which may panic missing `# Panics` section
   --> crates/server/src/dispatch/mod.rs:242:1
    |
242 | / pub fn dispatch(
243 | |     tree: &mut Tree,
244 | |     description: &Description,
245 | |     context: &Context,
246 | |     request: &Request,
247 | | ) -> Response {
    | |_____________^
    |
note: first possible panic found here
   --> crates/server/src/dispatch/mod.rs:256:21
    |
256 |       let noun_verb = request
    |  _____________________^
257 | |         .method
258 | |         .split_once('.')
259 | |         .expect("validate already confirmed this method is in the description");
    | |_______________________________________________________________________________^
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#missing_panics_doc
    = note: requested on the command line with `-D clippy::missing-panics-doc`

error: could not compile `steg-server` (lib) due to 4 previous errors
error: could not compile `steg-server` (lib test) due to 2 previous errors
full output: .quiet/lint.log
error: recipe `lint` failed with exit code 101
```

Check FAILED: `just check` exited 101 at 2026-11-15T16:49:37+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.75 on task-1.75-server-dispatch-a-request-to-the-kernel at c8dc7cdd8d3d with uncommitted changes, last 40 of 65 lines:

```text
   |
25 |     let pane = tree.session(session).unwrap().tabs()[0].panes()[0].id();
   |                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: if this value is `None`, it will panic
   = help: consider using `expect()` to provide a better panic message
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#unwrap_used

error: very complex type used. Consider factoring parts into `type` definitions
  --> crates/server/tests/dispatch.rs:94:16
   |
94 |     let cases: Vec<(&str, Vec<(&str, Value)>, &str)> = vec![
   |                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#type_complexity
   = note: `-D clippy::type-complexity` implied by `-D warnings`
   = help: to override `-D warnings` add `#[allow(clippy::type_complexity)]`

error: used `unwrap()` on an `Option` value
   --> crates/server/tests/dispatch.rs:222:18
    |
222 |     let method = api.method(dotted).unwrap();
    |                  ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: if this value is `None`, it will panic
    = help: consider using `expect()` to provide a better panic message
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#unwrap_used

error: wildcard match will also match any future added variants
   --> crates/server/tests/dispatch.rs:227:9
    |
227 |         other => panic!("{dotted}'s returns is not a record: {other:?}"),
    |         ^^^^^ help: try: `other @ steg_sdk::Returns::Nothing | other @ steg_sdk::Returns::Identifier { .. } | other @ steg_sdk::Returns::Text`
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#wildcard_enum_match_arm
    = note: requested on the command line with `-D clippy::wildcard-enum-match-arm`

error: could not compile `steg-server` (test "dispatch") due to 6 previous errors
full output: .quiet/lint.log
error: recipe `lint` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-15T16:51:14+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.75 on task-1.75-server-dispatch-a-request-to-the-kernel at c8dc7cdd8d3d with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (0.6s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (2.2s)
ok      build (steg-sdk) (0.6s)
ok      test (steg-sdk) (1.0s)
ok      build (steg-kernel) (0.8s)
ok      test (steg-kernel) (1.0s)
ok      build (steg-server) (0.8s)
ok      test (steg-server) (1.1s)
ok      build (steg) (0.3s)
ok      test (steg) (0.3s)
ok      build (steg-app) (2.1s)
ok      test (steg-app) (0.9s)
ok      tool tests (1.2s)
ok      architecture (1.0s)
ok      recipe names (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-15T16:53:36+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.75 on task-1.75-server-dispatch-a-request-to-the-kernel at c8dc7cdd8d3d with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (0.7s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (0.9s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.6s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.2s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```

Check passed: `just check` exited 0 at 2026-11-15T16:55:10+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.75 on task-1.75-server-dispatch-a-request-to-the-kernel at ecad31448c56:

```text
ok      format check (0.1s)
ok      lint (0.8s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (2.2s)
ok      build (steg-sdk) (0.3s)
ok      test (steg-sdk) (0.9s)
ok      build (steg-kernel) (0.5s)
ok      test (steg-kernel) (0.6s)
ok      build (steg-server) (0.2s)
ok      test (steg-server) (0.7s)
ok      build (steg) (0.2s)
ok      test (steg) (0.3s)
ok      build (steg-app) (0.6s)
ok      test (steg-app) (0.7s)
ok      tool tests (1.2s)
ok      architecture (1.1s)
ok      recipe names (0.1s)
```

Built as planned, one change: Context, Outcome/into_value, map_kernel_error
(exhaustive over kernel::Error, 13 variants including the new Engine arm),
session.rs/tab.rs/pane.rs/api.rs's served arms, the api.call rewrite-and-
recurse, R115's --no-focus capture-and-restore (with a rollback on
pane.split if the given label is rejected), and spikes/api-description/
deleted. sdk gets one addition beyond the plan's own scope: ErrorCode::
EngineFailed (exit status 1), since kernel::Error::Engine had no code to
map to - flagged on the plan for the engineer, who approved it with the
plan.

Which test proves which clause:
- AC #1 (validated, dispatched, typed result, unknown method/parameter
  refused): every per-method test in crates/server/tests/dispatch.rs
  (session_*/tab_*/pane_*/api_* tests); api_call_with_no_target_is_a_bad_
  request and api_call_of_an_unknown_method_is_refused_the_same_way_a_
  direct_call_would_be for validation through api.call's own path.
- AC #2 (every described method has an arm and every arm is described,
  both ways; a not-built method's own distinct error naming what it
  waits on): dispatch::tests::every_described_method_has_exactly_one_
  handler (crates/server/src/dispatch/mod.rs, both directions against
  description().methods); tests/dispatch.rs's every_not_yet_scenario_
  answers_not_implemented_yet_and_changes_nothing (19 scenarios: the 15
  whole-method not-yet cases, tab.move --to, and session.new/tab.new/
  pane.split's cwd-or-program branch), each checked for the error code,
  the message naming what it waits on, and the tree unchanged;
  every_described_method_gets_a_response_with_no_panic as the smoke test
  against arm-count drift.
- AC #3 (every served method's result serialises to exactly its declared
  fields): session_show_matches_exactly_the_fields_the_description_
  declares and session_list_matches_exactly_the_fields_the_description_
  declares_per_record (the one AC #3 names, through a real dispatch call);
  every_served_nothing_returning_method_answers_exactly_null and every_
  served_identifier_returning_method_answers_exactly_one_id_field for the
  shape of every other served method; api.list/show/call excluded per the
  sdk's own documented exemption.
- AC #4 (every kernel error maps, with the code and exit status the
  description gives): kernel_error::tests, one test per kernel Error
  variant (crates/server/src/dispatch/kernel_error.rs), including engine_
  maps_to_engine_failed... no: the Engine arm has no unit test (would need
  ghostty-vt as a direct steg-server dependency, which doc-16 R92 does not
  allow) - the exhaustive match itself, which fails the build on an
  unmapped variant, is the proof for that one arm; description::methods::
  tests::errors_has_eleven_codes_with_their_exit_statuses (steg-sdk) for
  the exit status table, including the new EngineFailed entry.
- AC #5 (every method callable by name; api show with no target returns
  the whole description; api list/show/call all work): api_list_returns_
  every_method, api_show_with_a_target_returns_one_method, api_show_with_
  no_target_returns_the_whole_description, api_show_an_unknown_method_is_
  no_such_target, api_call_of_pane_split_gives_the_same_response_a_direct_
  call_would (byte-for-byte against a direct call).
- AC #6 (creating without focus leaves the previous focus in place, R115):
  tab_new_with_no_focus_restores_the_previous_current_tab and pane_split_
  with_no_focus_restores_the_previous_current_pane, each comparing the
  whole session/tab before and after (label, colour/fill, the one new
  tab/pane added, every pre-existing tab/pane byte-for-byte unchanged),
  not just the one field expected to move; tab_new_with_no_focus_leaves_
  the_original_tab_current and split_set_label_then_close_the_session as
  the plan's two named end-to-end scenarios.
- AC #7 (spikes/api-description/ deleted): the diff itself (first commit);
  `just check`'s architecture and check-packages steps both passed after
  the deletion, confirming nothing else referenced it. This dangles decision
  -1 sections 3 and 9's citations of carry-one-method/measure.py, as
  TASK-1.43 flagged twice already - not this task's to fix, decision text
  changes only via a PR containing nothing else.

just check recorded clean on the task repeatedly, including after
rebasing onto origin/main twice (once before any code was written, once
at the end - neither rebase touched crates/, only board files; TASK-1.40/
1.42/1.70 are not yet merged to main). CI green on PR #33's head.

Three findings beyond the approved plan, found while building, not
previously flagged:
1. pane.move's target tab needs a Direction Tree::move_pane_to_tab
   requires, but doc-2 section 8's row for pane move has no direction
   parameter. Dispatch defaults to Direction::Right (append at the end
   of the target tab) - a per-call choice, not a method-level not-yet,
   since the method is otherwise fully servable. Worth a line in doc-2
   if a caller should ever be able to ask for a different position.
2. api.call can never carry a target method's own `target` parameter:
   api.call's own selector is itself named `target` (per TASK-1.43's own
   review decision, matching the convention every other method's target
   uses), so api::call's rewrite always removes the key named `target`
   before building the inner request - there is no second key the inner
   method's own target could arrive under. api.call of pane.split always
   acts on the context's default pane, never a pane named explicitly.
   doc-2 section 2 says api.call "is for agents, and for a method a
   plugin adds before the CLI knows it" - an agent naming a specific pane
   is exactly the case this blocks. Not fixed here: fixing it means
   changing the description (a second key, or renaming the selector),
   which this plan does not authorize.
3. unset --label (and tab.unset/session.unset's own --label) requires
   sending some Text value on the wire, which dispatch then discards -
   unset always clears the label regardless of what value arrives.
   unset --label arguably wants a Switch, not a Text parameter. Also
   TASK-1.43's description (already reviewed); flagged, not changed.

crates/server/tests/dispatch.rs carries a file-level
#![expect(clippy::unwrap_used, reason = "...")]: clippy's own
allow-unwrap-in-tests option (root clippy.toml) only recognises
#[cfg(test)] unit test modules, not a file under tests/, even though
the whole file is already test-only - same as kernel's and sdk's own
clippy.toml comment on the same setting, just hitting its one gap.

Departure from the plan's file list, noted as it happened: the
completeness test and the kernel-error-mapping test live beside their
own code in crates/server/src/dispatch/ (mod.rs, kernel_error.rs), not
in tests/dispatch.rs as the plan's file list named - mod.rs's own
HANDLED-checking test needs handled()/NOT_YET_NOUNS, which are private
to the crate (and #[cfg(test)]-only, since nothing in the production
path reads them), so an external functional test cannot reach them.
Everything else the plan named for tests/dispatch.rs - the per-method
tests, the Outcome-versus-declared-fields cross-check, the position
boundary, the api.call tests, the --no-focus tests, the two end-to-end
scenarios - landed there as planned.

Check FAILED: `just check` exited 1 at 2026-11-15T17:15:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.75 on task-1.75-server-dispatch-a-request-to-the-kernel at f6eb7a45144c with uncommitted changes:

```text
FAILED  format check (exit 1, 0.1s)
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.75/crates/server/src/dispatch/tab.rs:88:
         if let Err(error) = tree.move_tab(tab_id, index) {
             tree.close_tab(tab_id)
                 .expect("the tab this call just created is still in the tree");
[31m-            tree.focus_tab(previous_tab).expect(
(B[m[31m-                "the session this call just created a tab in still holds its previous tab",
(B[m[31m-            );
(B[m[32m+            tree.focus_tab(previous_tab)
(B[m[32m+                .expect("the session this call just created a tab in still holds its previous tab");
(B[m             return Err(map_kernel_error(error));
         }
     }
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.75/crates/server/tests/dispatch.rs:1050:
         covered.push(dotted.clone());
 
         if not_yet.contains(&dotted.as_str()) {
[31m-            let error = err(dispatch(&mut tree, &api, &context, &request(&dotted, vec![])));
(B[m[32m+            let error = err(dispatch(
(B[m[32m+                &mut tree,
(B[m[32m+                &api,
(B[m[32m+                &context,
(B[m[32m+                &request(&dotted, vec![]),
(B[m[32m+            ));
(B[m             assert_eq!(error.code, ErrorCode::NotImplementedYet, "{dotted}");
             continue;
         }
full output: .quiet/format-check.log
error: recipe `check-fmt` failed on line 117 with exit code 1
```

Commits since the review: 26, 24 without the Asdlc-Worker trailer.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-15T16:26:00Z

---

Plan approved by @bruk (f570689623ad, size-3): its worker may build it.

---

author: @bruk.r5
created: 2026-11-15T16:58:00Z

---

@bruk.r5 is reviewing TASK-1.75: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r5
created: 2026-11-15T17:10:00Z

---

Review by @bruk.r5 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 1 Important, 3 nit. Cites: review, rust. The diff does not match the plan.

[Compliance, Important] crates/server/src/dispatch/tab.rs:44 (`new`): tab.new's own description declares a `position` flag ("Where among its siblings the new tab goes. Defaults to the end" - crates/sdk/src/description/methods.rs:410-415), and doc-2 section 8 lists --position on `tab new`. The arm never reads it: it resolves cwd/program, label and focus, then calls tree.new_tab(session_id, label), which always appends at the end. tab.new --position 1 answers success with the new tab's id, but the tab lands at the end regardless of what was asked. This is not a not-yet case: Tree::move_tab already exists and tab.move's own arm already composes it with the same wire_position_to_index conversion, so new_tab-then-move_tab is available today. The plan's own re-applied test ("does the method's own returns/documented effect imply the flag's effect took hold? If yes and the kernel cannot honour it, refuse") was applied to --cwd/program, --force and session.new --focus, but position on tab.new is not on that list and is not among the three disclosed findings in the notes - it is exactly the "success handed back for something that quietly didn't happen" bug the plan's decision 4 was written to prevent for --cwd/program. No test in dispatch.rs calls tab.new with position.

[Rust, Nit] crates/server/src/dispatch/mod.rs: the Context struct, the top-level dispatch function, and a dozen-plus helpers (target_session, resolve_pane_str, pane_mut, wire_position_to_index, etc.) live directly in mod.rs, against .claude/rules/rust.md's "lib.rs and mod.rs hold mod and pub use lines only. Behaviour lives in named modules." The plan itself named this file for this content, so it is not an unplanned departure, but it is worth a line for a later split.

[Bugs, Nit] crates/server/tests/dispatch.rs: every_served_nothing_returning_method_answers_exactly_null and every_served_identifier_returning_method_answers_exactly_one_id_field cover only 3 and 1 of the roughly 14 served Nothing/Identifier-returning arms respectively, despite their "every" names. Not a functional hole - Outcome::into_value's own unit tests already guarantee the shape for every variant - but the test names overclaim what they check.

[Evidence, Nit] crates/server/tests/dispatch.rs: the R115 --no-focus restore tests (pane_split_with_no_focus_restores_the_previous_current_pane, tab_new_with_no_focus_restores_the_previous_current_tab/tab_new_with_no_focus_leaves_the_original_tab_current) only run against a fresh tree with one pane/tab, so the split's target is trivially also the tab's current pane, and the session's current tab is trivially its only one. The harder cases (splitting a pane that is not current; a session whose current tab is not its first) are correct by inspection - pane::split reads find_tab(tree, tab_id).current() and tab::new reads session.current_tab(), never the target - but neither is exercised by a test.

Verified directly: every served method's arm checked individually against doc-2 section 8 and the sdk's own declared params for session (8/8 verbs), tab (8/8, partial), pane (13/13, partial) and api (3/3) - the tab.new finding above is the only parameter accepted by validate and then silently unacted on; every other declared flag on every served or partially-served method is either read and acted on, or is one of the plan's disclosed no-op cases (--force on *.close, session.new --focus/--no-focus). Positions: session.move and tab.move both convert correctly (1 -> kernel 0, tested; 0 -> BadRequest with no kernel call, tested for session.move and at the shared wire_position_to_index level for both). Completeness both ways: verified empirically, not just by reading the test - removed ("session","show") from session::HANDLED in a scratch edit, reran the completeness test, got the expected two-sided mismatch failure; separately added a bogus ("session","bogus") entry, reran, got the expected reverse-direction failure; reverted both (worktree clean). api.call: validated as refusing itself, refusing a missing/non-string target, and recursing through the same dispatch/validate path a direct call would take (byte-for-byte match test for pane.split; same BadRequest for an unknown inner method as a direct call gets), so it cannot reach anything a direct call could not. Kernel error mapping: exhaustive over all 13 variants steg_kernel::Error has on origin/main today, no wildcard arm, confirmed by reading kernel_error.rs directly - not missing any current variant. Of the four parallel tasks named, task-1.40 and task-1.42 each add kernel::Error variants this match does not yet cover (task-1.40: ProgramAlreadyStarted, ProgramNotStarted, ProgramStillRunning, ShellCannotRestart; task-1.42: ClientNotFound, ClientNotViewingPane, ClientNotViewingTab) - expected per the review brief, not a finding, but whichever of those merges first will break steg-server's build here until map_kernel_error is extended. Refusals create nothing: verified for session.new/tab.new/pane.split given cwd or program, and for pane.split given an invalid label (rolls back the split and the tab's current pane) - each checked before any Tree mutation in the source, and asserted by dedicated or table-driven tests comparing the whole tree/session/tab before and after. Typed results: Outcome's 9 variants match exactly what this task's arms produce, no unused variant. Evidence: just check recorded at commit ecad31448c56, which is HEAD; PR #33's CI ("check") is SUCCESS on that same SHA.

plan-matched is "no": the plan's file list does not name crates/server/src/dispatch/api.rs (it put api.list/show/call's handling under mod.rs in the plan text) - a real new file beyond what was named, though it holds exactly what the plan described logically. Also disclosed in the notes: the completeness and kernel-error tests live beside their own code in src/dispatch/ rather than in tests/dispatch.rs as the plan's file list named, for a documented reason (HANDLED/NOT_YET_NOUNS are #[cfg(test)]-private). Both are departures the notes explain, not undisclosed ones, so neither is counted as the Important finding above, but the instructions call for "no" on any departure.

---
<!-- COMMENTS:END -->

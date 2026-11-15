---
id: TASK-1.81
title: api call cannot reach a method that has its own target
status: In Progress
assignee:
  - '@bruk.w2'
created_date: '2026-11-15T20:14:00Z'
updated_date: '2026-11-15T21:11:00Z'
labels:
  - size-2
milestone: m-3
dependencies:
  - TASK-1.48
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-5
asdlc_branch: task-1.81-api-call-cannot-reach-a-method-that-has
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'd5c2871b65a2'
asdlc_plan_approved_size: size-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice: TASK-1.75's worker noted that api.call can never carry the called method's own target, because api.call's selector is itself the parameter named target; TASK-1.48's worker then showed the consequence is not only that a default is always used: a method whose target is required (setting.show, set, unset; client.show once built) cannot be called through api.call at all, since dispatch/api.rs removes the key before building the inner request and validate then refuses the inner call for a missing target. For session, tab and pane methods it silently acts on the context's default instead of the one named. doc-2 sections 2 and 8, decision-1 (the API describes itself and any method can be called by name; R36, R37). Matters for M1a because api call is how a script or a plugin reaches any method.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 api call can call every method in the description with every parameter that method declares, including the method's own target: shown by a test driven from the description that calls each served method through api.call with its target given and compares the response with a direct call
- [ ] #2 setting.show, setting.set and setting.unset, which have a required target and no default, work through api.call
- [ ] #3 How the method's name and the inner method's parameters are kept apart is decided in the plan against doc-2 (api call takes the method as its target and the parameters as JSON on standard input) and decision-1, and any change to the description or to doc-2 is named as its own item for the engineer
- [ ] #4 A call through api.call that names a pane, tab or session acts on that one, not on the context's default
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## The decision (AC #3)

Three shapes were weighed for keeping api.call's own selector apart from the
called method's own parameters, which today collide because both are the
wire key `target` in `request.params`.

**(a) Nest the inner method's parameters under a second key** (e.g.
`params`, holding a JSON object), api.call's own selector staying `target`.
Rejected: the sdk's `ValueKind` enum (Text, Integer, Boolean, Choice, Target,
Path, Argv — `crates/sdk/src/description/request.rs`'s `matches_kind`) has
no case for an arbitrary JSON object, so this needs a new `ValueKind`
variant — a bigger decision-1 format change than the bug warrants — and the
CLI generator (TASK-1.49) would have to special-case rendering it, since
doc-2 section 8's own row for `api call` already describes the parameters
as arriving whole on standard input, not as a `--params <json>` flag.

**(b) A separate top-level field on `Request` itself** (outside `params`)
for the selector. Rejected: `Request`'s shape (`method`, `params`) is
decision-2/doc-14's wire contract, not this method's; changing it reaches
further than one method's bug.

**(c) Rename api.call's own selector field** from `target` to `method`,
keeping role `Target` (so it is still the CLI's one positional) and leaving
every other parameter flat, exactly as today. **Chosen.**
`Method::target()` (`crates/sdk/src/description/shapes.rs:119`) finds the
positional by `role == Role::Target`, never by name, so nothing about how
the CLI will build the command (TASK-1.49) or how `doc_2_section_8_targets()`
checks it (`crates/sdk/src/description/methods.rs` tests) depends on the
field being named `target`. Once the selector is `method`, the called
method's own `target` (if it declares one, e.g. `pane.split`, `setting.show`)
is a different key in the same flat map and survives untouched — the bug is
gone with no new `ValueKind`, no new `Role`, and no wire change.

Doc-2 needs no edit: section 8's Target column for `api call` already reads
"method" (not "target"), and section 2's "its target parameter is the
positional" / "parameters... read as one JSON object on standard input"
both stay true of the renamed field. decision-1 needs no edit: its format
already lets a parameter's wire `name` differ from its `role`. **Named for
the engineer as its own item, so it is clear nothing in `backlog/docs/` or
`backlog/decisions/` is asked to change:** the only change is the `Param`
literal for `api.call` in `crates/sdk/src/description/methods.rs` (its
`name` and its `details` prose) — code, not a spec or decision document.

`api.show`'s own `target` (also a bare method name, by convention) is left
unchanged: it has no other parameters to collide with, so renaming it buys
nothing and widens the diff. Said in the notes so a reviewer doesn't ask.

## What the orchestrator's case list reduces to under (c)

Two of the four named cases do not arise under this shape, and the notes
say so explicitly rather than inventing a test for them:

- **"inner parameters that are not an object"** — there is no nested value
  to be the wrong shape. `Request.params` is `Map<String, Value>`
  (`crates/sdk/src/description/request.rs`); the wire's own deserialization
  already refuses anything that is not a JSON object before `dispatch` is
  ever called. The only place this question has content is the far side of
  TASK-1.49's CLI (stdin that doesn't parse as a JSON object at all), which
  is that task's to test, not this one's.
- **"a method with no parameters at all (nested value absent versus
  empty)"** — there is no nested value, so no absence/empty distinction.
  Calling `session.list` through api.call is `{"method": "session.list"}`
  with nothing else, same as any method with no extra parameters.

The other two keep their own tests (below): an inner method unknown, and
api.call calling api.call.

## Files that change

1. `crates/sdk/src/description/methods.rs`
   - `api_methods()`: `api.call`'s `params` becomes a new helper, not
     `target_of_kind`, since every other call site of `target_of_kind`/
     `target`/`required_target` must keep the literal name `target` (they
     are all real session/tab/pane/setting targets). Add a small sibling
     function next to them:
     ```rust
     /// Role: Target parameter named `method`, for `api.call`'s own
     /// selector only (TASK-1.81) — not the generic `target` convention,
     /// because api.call forwards every other parameter flat into the
     /// called method's own `params`, and that method may itself declare
     /// a parameter literally named `target` (`pane.split`, `setting.show`,
     /// ...). Renaming the selector, not nesting the forwarded parameters,
     /// is what keeps the two apart.
     fn method_target(summary: &str) -> Param {
         Param {
             name: "method".into(),
             role: Role::Target,
             value: ValueKind::Text,
             required: true,
             summary: summary.into(),
         }
     }
     ```
   - `api.call`'s `Method` literal: `params: vec![method_target("The method
     to call, by its dotted name")]`, and update its `details` string to
     say the selector is `method`, not `target`, and that every other
     parameter given is the called method's own, by its own declared names,
     including that method's own `target` where it has one.
   - Add one test beside `no_method_declares_a_cli_only_flag_as_a_parameter`:
     no method other than `api.call` declares a parameter named `method` —
     the invariant that keeps the flat merge safe for every method added
     later, the same way that existing test keeps the CLI-only flags safe.
   - `doc_2_section_8_targets()` and
     `every_method_has_a_target_exactly_as_doc_2_section_8_gives_it` need no
     change: both read `described.target()` (role-based) and doc-2's
     Target-column text, neither of which names the wire field.

2. `crates/server/src/dispatch/api.rs`
   - `call()`: read the selector with `param_str(request, "method")`
     instead of `"target"`; the removal becomes `params.remove("method")`;
     the self-call check becomes `if method_name == "api.call"`. Keep the
     same `BadRequest` shape for an absent/wrong-typed selector, wording
     updated to name `method`.
   - Update the module doc comment (lines 1–19) and `call`'s own doc comment
     (lines 48–55), which currently narrate the collision in terms of
     `target`: they now describe the `method`/forwarded-`params` split and
     why it no longer collides. Leaving them as they are would describe a
     design the code no longer has.

3. `crates/server/tests/dispatch.rs` — tests, listed under Proof below.

Nothing in `crates/kernel`, `crates/app`, or any other crate reads
api.call's params directly, so this stays inside `steg-sdk` and
`steg-server`, matching both crates' `CLAUDE.md` (sdk depends on nothing,
server may depend on kernel/sdk) — no boundary crossed.

## For TASK-1.49 (not built; stated for whoever builds it, not acted on now)

Doc-2 section 2's mechanical transform (a method's Target-role parameter
becomes the one positional, whatever its wire name) already produces
`steg api call <method>` correctly from a parameter literally named
`method`, with no special case in the generator. The CLI reads stdin as a
JSON object (doc-2 section 2/8, unchanged) and sends
`Request { method: "api.call", params: {"method": <name>, ...stdin's own
keys>} }` — the same flat shape the server now expects. A plugin or script
calling the wire protocol directly builds the same shape by hand.

## Proof

Driven from the description so a new method cannot be missed, same idiom
as `every_nothing_or_identifier_returning_method_matches_its_declared_shape`
(`crates/server/tests/dispatch.rs:1211`): a `match` on every method's dotted
name building that method's own extra parameters, with a `panic!` fallback
naming any method the match doesn't cover.

- **New test, AC #1 and #4 together**: for every method in the description
  except `api.call` itself (excluded by name, with the comment "refuses to
  call itself" pointing at its own test below — not a silent filter), call
  it directly and through `api.call` on two freshly built, identical trees
  (`fresh()`, twice, as `api_call_of_pane_split_gives_the_same_response_a_
  direct_call_would` already does), giving an explicit non-default target
  where the method has one (a freshly created sibling session/tab/pane, not
  the context's default — same shape as the existing per-method match's
  `session_to_close`/`tab_to_move`/... locals), and assert the two
  `Response`s are equal.
- **AC #4's other half, its own tests** (response equality alone does not
  prove this: both calls could produce `Response::Ok(Nothing)` even if one
  silently acted on the default instead of the named target). One test per
  noun that can be named and changed — `pane.set`, `tab.set`, `session.set`
  — each creating a second (non-default) pane/tab/session beside the
  context's default, calling it through `api.call` with the named one as
  the target, and asserting both halves: the named one changed, and the
  context's default did not.
- **Settings by name, their own tests** (not folded into the general loop,
  since these are the two cases TASK-1.48's worker found and the task's
  notes ask to be shown explicitly): `setting.show`, `setting.set`,
  `setting.unset` through `api.call`, each compared against a direct call,
  mirroring `api_call_of_pane_split_gives_the_same_response_a_direct_call_
  would`.
- **Inner method unknown**: strengthen the existing
  `api_call_of_an_unknown_method_is_refused_the_same_way_a_direct_call_
  would_be` to compare the whole `ApiError` (code, message, candidates)
  against a real direct call's error, not only `error.code` — today it
  only checks the code, which would still pass even if the message or
  candidates diverged between the two paths.
- **api.call calling api.call**: update the existing
  `api_call_refuses_to_call_itself` to send `{"method": "api.call"}`.
- **Inner request fails validation**: new test — call `api.call` with a
  method and parameters that fail `validate` (e.g. `pane.split` with no
  `direction`, its one required parameter), and assert the `ApiError` is
  identical, field for field, to the error a direct call with the same
  (invalid) parameters gives. This is the case the task names explicitly
  ("the error must be the one a direct call gives") and is not covered by
  the general parity loop above, which only exercises valid calls.
- **Method with no parameters at all**: covered implicitly by the general
  loop (`session.list`, `api.list`, `server.show`, ... all have none), and
  explained rather than specially tested, per "what dissolves" above.

### The deliberate failing run

Before any production code changes, the new/updated tests are written
first and run against today's code (`api::call` still reading `"target"`).
Expected failure: every test that builds a request with the new `"method"`
key gets back `BadRequest`, message `"target: must be text"` (today's
wording) — not because the comparison logic is wrong, but because
`api::call`'s own `let Some(target) = param_str(request, "target")` finds
nothing (the request no longer carries a `target` key at all) before it
ever reaches the method it's meant to call. Recorded with
`asdlc record-check` before the rename lands, then again after, both in
the task's worktree, through `just test` (server crate, since the diff
touches `crates/server` and `crates/sdk`) and `just check` before calling
it done.

## Risks

- The general parity loop's biggest risk is a method whose response is not
  deterministic across two separately-built trees (same risk the existing
  `pane.split` test already accepts via `Tree::new()`'s deterministic ids).
  Scanned `crates/sdk/src/description/methods.rs`'s 42 methods: nothing
  returns a clock-derived or random value in M1's served set, so this holds;
  flagged here rather than assumed silently.
- `NotImplementedYet` methods (the `server`/`event`/`client` arms not yet
  built, plus whichever of `session`/`tab`/`pane` TASK-1.75's own `NotYet`
  cases name) are left in the general loop rather than excluded: calling
  one through `api.call` should give the exact same `NotImplementedYet`
  error a direct call gives, which is itself useful evidence that `api.call`
  forwards errors transparently, not just successes.

## Scope

One reviewable change: one field rename in one `Param` literal (plus its
prose), two reads in `crates/server/src/dispatch/api.rs`, and tests. No
split proposed.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TASK-1.48 added two tests in crates/server/tests/dispatch.rs that record today's behaviour (api_call_cannot_reach_setting_show_because_its_own_target_is_also_named_target, api_call_cannot_reach_setting_set_for_the_same_reason). The fix inverts them: say so in the notes rather than deleting them quietly.

Correction: the two tests mentioned above were removed from TASK-1.48 before it merged (ae89d2b), so there is nothing to invert; this task writes the tests that show api.call reaching the settings methods.

Plan checked against doc-2 (sections 2, 5, 8), decision-1 and doc-14 by @bruk.o: ok. api.call's own selector is renamed from target to method on the wire and keeps the Target role, so it is still the CLI's one positional and doc-2's row (target: method) is already true; the called method's parameters stay flat beside it, so its own target survives. No spec or decision document changes; the description of api.call changes (one parameter's name), which the engineer approves with the plan. Nesting the parameters was weighed and set aside as a larger change to the description format. A test fixes that no method declares a parameter named method. Proof is driven from the description, with named targets shown to be acted on rather than the default.

Check passed: `just check` exited 0 at 2026-11-15T21:05:13+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.81 on task-1.81-api-call-cannot-reach-a-method-that-has at 4567a1a52ab8:

```text
ok      format check (0.1s)
ok      lint (82.0s)
ok      boundaries (0.2s)
ok      licences (0.8s)
skip    tests: this change touches no crate
ok      build (steg-sdk) (3.1s)
ok      test (steg-sdk) (1.8s)
ok      test (steg-sdk, local-socket) (2.0s)
ok      build (steg-kernel) (43.8s)
ok      test (steg-kernel) (1.3s)
ok      build (steg-server) (43.8s)
ok      test (steg-server) (2.4s)
ok      build (steg) (0.2s)
ok      test (steg) (0.3s)
ok      build (steg-app) (18.3s)
ok      test (steg-app) (0.8s)
ok      tool tests (1.3s)
ok      architecture (0.8s)
ok      recipe names (0.1s)
```

Check FAILED: `sh -c 'cd crates/server && cargo test --test dispatch'` exited 101 at 2026-11-15T21:10:20+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.81 on task-1.81-api-call-cannot-reach-a-method-that-has at 4567a1a52ab8 with uncommitted changes, last 40 of 72 lines:

```text
test tab_new_with_no_focus_restores_the_previous_current_tab ... ok
test api_call_refuses_to_call_itself ... ok
test api_call_with_no_target_is_a_bad_request ... ok
test api_show_an_unknown_method_is_no_such_target ... ok
test tab_new_with_position_zero_is_refused_and_creates_nothing ... ok
test tab_new_with_a_middle_position_places_the_new_tab_there ... ok
test setting_unset_an_unknown_name_is_no_such_target ... ok
test tab_new_with_a_position_past_the_end_and_no_focus_creates_nothing_and_restores_focus ... ok
test tab_new_with_position_one_places_the_new_tab_first ... ok
test pane_close_removes_the_pane ... ok
test tab_move_with_position_reorders ... ok
test pane_split_makes_the_new_pane_current_by_default ... ok
test pane_focus_moves_to_the_neighbour ... ok
test pane_split_with_a_program_refuses_and_creates_nothing ... ok
test pane_set_zoomed_fills_and_unset_zoomed_unfills ... ok
test tab_new_makes_the_new_tab_current_by_default ... ok
test pane_move_moves_it_to_another_tab ... ok
test tab_new_with_a_position_past_the_end_behaves_like_tab_move_and_creates_nothing ... ok
test tab_focus_makes_it_current ... ok
test api_list_returns_every_method ... ok
test every_nothing_or_identifier_returning_method_matches_its_declared_shape ... ok
test api_show_with_no_target_returns_the_whole_description ... ok
test every_not_yet_scenario_answers_not_implemented_yet_and_changes_nothing ... ok
test every_described_method_gets_a_response_with_no_panic ... ok

failures:

---- api_call_of_pane_split_gives_the_same_response_a_direct_call_would stdout ----

thread 'api_call_of_pane_split_gives_the_same_response_a_direct_call_would' (13322216) panicked at crates/server/tests/dispatch.rs:1182:18:
expected both calls to succeed: (Ok(Object {"id": String("00000003")}), Error(ApiError { code: BadRequest, message: "method: must be text", candidates: [] }))
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    api_call_of_pane_split_gives_the_same_response_a_direct_call_would

test result: FAILED. 51 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

error: test failed, to rerun pass `--test dispatch`
```

Check FAILED: `just check` exited 1 at 2026-11-15T21:11:34+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.81 on task-1.81-api-call-cannot-reach-a-method-that-has at 4567a1a52ab8 with uncommitted changes, last 40 of 64 lines:

```text
(B[m[32m+                ("label", json!("")),
(B[m[32m+            ]
(B[m         }
 
         "pane.list" => {
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.81/crates/server/tests/dispatch.rs:1596:
         "client.show" => vec![("target", json!("x"))],
 
         "setting.show" | "setting.unset" => vec![("target", json!("appearance"))],
[31m-        "setting.set" => vec![
(B[m[31m-            ("target", json!("appearance")),
(B[m[31m-            ("value", json!("light")),
(B[m[31m-        ],
(B[m[32m+        "setting.set" => vec![("target", json!("appearance")), ("value", json!("light"))],
(B[m 
         "api.show" => vec![("target", json!("pane.split"))],
 
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.81/crates/server/tests/dispatch.rs:1667:
             &request("api.call", via_params),
         );
 
[31m-        assert_eq!(canonical(direct_response), canonical(via_response), "{dotted}");
(B[m[32m+        assert_eq!(
(B[m[32m+            canonical(direct_response),
(B[m[32m+            canonical(via_response),
(B[m[32m+            "{dotted}"
(B[m[32m+        );
(B[m     }
 
[31m-    assert_eq!(checked, api.methods.len() - 1, "api.call is the one exclusion");
(B[m[32m+    assert_eq!(
(B[m[32m+        checked,
(B[m[32m+        api.methods.len() - 1,
(B[m[32m+        "api.call is the one exclusion"
(B[m[32m+    );
(B[m }
 
 // --- shapes: Nothing/Identifier (AC #3) ------------------------------------
full output: .quiet/format-check.log
error: recipe `check-fmt` failed on line 126 with exit code 1
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-15T21:01:00Z

---

Plan approved by @bruk (d5c2871b65a2, size-2): its worker may build it.

---
<!-- COMMENTS:END -->

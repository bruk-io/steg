---
id: DRAFT-5
title: api call cannot reach a method that has its own target
status: Draft
assignee: []
created_date: '2026-11-15T20:14:00Z'
updated_date: '2026-11-15T20:23:00Z'
labels:
  - size-2
milestone: m-3
dependencies:
  - TASK-1.48
parent_task_id: TASK-1
project: bruk-io-steg
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TASK-1.48 added two tests in crates/server/tests/dispatch.rs that record today's behaviour (api_call_cannot_reach_setting_show_because_its_own_target_is_also_named_target, api_call_cannot_reach_setting_set_for_the_same_reason). The fix inverts them: say so in the notes rather than deleting them quietly.

Correction: the two tests mentioned above were removed from TASK-1.48 before it merged (ae89d2b), so there is nothing to invert; this task writes the tests that show api.call reaching the settings methods.
<!-- SECTION:NOTES:END -->

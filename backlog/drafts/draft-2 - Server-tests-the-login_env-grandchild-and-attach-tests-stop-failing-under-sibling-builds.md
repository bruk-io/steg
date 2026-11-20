---
id: DRAFT-2
title: >-
  Server tests: the login_env grandchild and attach tests stop failing under
  sibling builds
status: Draft
assignee: []
created_date: '2026-11-20T17:21:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split out of TASK-1.137 on its planner's recommendation (2026-11-20): the flake is small and urgent and touches only crates/server/src/login_env.rs's test and crates/server/tests/attach.rs, while the rest of TASK-1.137 is a sweep across most test files. Diagnosis already on TASK-1.137's plan, read it first: (1) login_env - the fixture forks before writing its pid file, and capture_login_environment's one-second functional timeout kills the process group when it expires; under heavy load the shell has not reached the write yet, so the later poll waits for a file that never appears. Proposed: raise that test's own timeout from 1 s to 5 s. (2) attach.rs - which tests fail was never recorded; just check runs four copies of the attach binary and the cross-process flood guard covers only five named tests. Sightings: TASK-1.137's notes, plus two more just check runs on 2026-11-20 (TASK-1.142's worker). CI stays green throughout, so this is a local-load fault, not a product fault.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The login_env backgrounded-grandchild test no longer depends on the pane's shell reaching its pid-file write within one second: a recorded control with a fixture that delays the write fails at the old bound and passes at the new one
- [ ] #2 The attach.rs tests that fail under concurrent builds are named from a recorded experiment (the four feature-variant binaries run together, with and without sibling build load), and the fix follows what the experiment shows
- [ ] #3 If the experiment cannot reproduce an attach.rs failure, the task says so plainly and applies only the stated default (the flood guard widened to the whole file), with that limit recorded
- [ ] #4 No test is weakened or skipped, no liveness bound is removed, and just check passes at the final head
<!-- AC:END -->

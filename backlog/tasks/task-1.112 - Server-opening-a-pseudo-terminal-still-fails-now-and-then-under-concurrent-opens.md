---
id: TASK-1.112
title: >-
  Server: opening a pseudo-terminal still fails now and then under concurrent
  opens
status: To Do
assignee: []
created_date: '2026-11-16T04:53:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Seen on 2026-11-16 (UTC) on main at d06ab92 (the merge of pull request 52, which changed only Python tools): crates/server/tests/real_pty.rs many_threads_opening_a_real_pty_at_once_never_fails failed at round 10, thread 27 of 32 with 'RealLauncher::spawn failed under concurrent opens: the pseudo-terminal failed: failed to openpty: Os { code: 6, Device not configured }', in the step that tests steg-server alone. The same code passed CI on pull request 47's head, which contains that commit. TASK-1.76 added the process-wide OPENPTY_LOCK around openpty on the orchestrator's decision (a lock and no retry) after its review isolated a race between concurrent openpty calls on macOS; this shows the lock is not the whole answer. It is a real defect and not only a flaky test: a user's pane could fail to open. crates/server/src/pty.rs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The cause of the failure seen in CI is established, not assumed: openpty returned 'Device not configured' (ENXIO) in many_threads_opening_a_real_pty_at_once_never_fails while the process-wide lock around openpty was held, so the race is not only between two openpty calls in one process. The plan finds what else it races with (the spawn that follows and opens the slave, a close of another pty's descriptors on another thread, another process on the machine, a limit) by experiment on macOS, and says which
- [ ] #2 The fix addresses that cause: for example the lock covering what the experiment shows it must cover, or a bounded retry of the open on this one error if the cause is outside the process and cannot be excluded; a retry is stated with its bound and its evidence, and is not a blanket retry of any error
- [ ] #3 A pane whose pseudo-terminal cannot be opened after that still fails cleanly with the error doc-2 gives, leaving no half-made pane, thread or child
- [ ] #4 Shown by repeated runs: the test passes N times in a row locally under load and on CI (a temporary workflow that repeats it, removed before merge), with N justified against the failure rate seen; and a negative control: with the fix removed, the experiment that reproduces the failure does so
<!-- AC:END -->

---
id: DRAFT-2
title: >-
  Rule: a spawned child process must be wait()ed on every path that can end it,
  not only the happy path
status: Draft
assignee: []
created_date: '2026-11-15T17:12:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.40
  - TASK-1.70
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: a child process this code spawns is reliably wait()ed on only in the
one path the worker was thinking about, and silently never reaped on another real path, in
TASK-1.40 and TASK-1.70.

- TASK-1.70 (sdk/src/local_socket.rs): connect()'s autostart spawns a server process and never
  wait()s on it at all - a Nit, since the real server detaches almost immediately.
- TASK-1.40 (server/src/pane.rs, spawn_relay): the relay thread does wait() on the child, but
  only when its own read loop runs to completion; if the pane's worker thread is gone first
  (PaneHandle::shutdown() while the program is still running, reachable via `pane close
  --force`), the thread returns on a failed channel send without ever calling child.wait(),
  leaving the child a zombie once it exits - confirmed by reproduction (a real `sleep 2`
  process observed as `<defunct>` in `ps` after shutdown()). The same early return also leaves
  the thread blocked forever in reader.read() when the program produces no output before the
  pane shuts down.

Add to plugins/team/rules/ (or a steg-specific rules file under plugins/bruk-io-steg/rules/,
if the team rule should stay generic): wherever code spawns a child process, trace every path
that can end its lifecycle - the happy path, an early return, an error branch, a shutdown
signal - and confirm each one still reaches a wait()/try_wait() call, not only the path the
author was picturing. A reviewer should ask explicitly "which code calls wait() on this child,
and on every path that drops its last handle, does that path still run?"
<!-- SECTION:DESCRIPTION:END -->

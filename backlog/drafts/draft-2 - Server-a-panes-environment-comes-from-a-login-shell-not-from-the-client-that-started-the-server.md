---
id: DRAFT-2
title: >-
  Server: a pane's environment comes from a login shell, not from the client
  that started the server
status: Draft
assignee: []
created_date: '2026-11-15T23:40:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.77
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Third piece of the split of TASK-1.77 (2026-11-15). doc-8 R99: a server started on demand 'takes its environment from a login shell and not from that client'. The worker's plan on TASK-1.77 proposed resolving only the login identity (shell, home, user) from the password database; that is needed but is not the requirement, since the server would still pass the starting client's environment on to every pane. TASK-1.76's Runtime takes one injected LoginShell value read today from the process environment as a named stand-in; this task replaces that and supplies the base environment. TASK-1.44's first plan (before its split) has an earlier design for capturing the environment with a cleared, seeded child and a bounded wait: read it. No unsafe: std::env::set_var is not to be used; the environment is data handed to the launcher.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The environment a pane's program starts with comes from a login shell of the person's account, found from the password database, and not from the environment of the client that happened to start the server (R99): a variable set only in that client's environment is absent from a new pane, and what a login shell exports (PATH above all) is present, for a shell pane and for a command pane, shown with a real server started on demand by a real client
- [ ] #2 Getting that environment cannot hang the server or delay its start beyond a stated bound on a slow, broken or interactive shell profile; what is used when it fails or times out is stated, never the client's environment unscrubbed, and every child started to get it is waited on along every path
- [ ] #3 The STEG_ variables a pane is given (doc-2 section 4) and TERM are set by the server whatever the login shell says; the working-directory rule of TASK-1.76 is unchanged
- [ ] #4 The plan weighs the ways to do it and picks one with reasons: capturing a login shell's environment once at server start; starting every pane's program through a login shell with a scrubbed environment; or both; and says what each means for a command pane, which runs no shell of its own
<!-- AC:END -->

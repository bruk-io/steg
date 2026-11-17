---
id: DRAFT-2
title: >-
  Server: a pane's program must not inherit a client's connection or the
  listener
status: Draft
assignee: []
created_date: '2026-11-17T01:33:00Z'
labels:
  - size-2
milestone: m-0
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by TASK-1.113's investigation on 2026-11-16, not yet shown in the product. On macOS the standard library marks an accepted socket close-on-exec with a second call after accept, not atomically, and the same for a socket pair. The server accepts client connections on one thread while pane programs are started on others. A program started in that gap inherits the client's connection, and possibly other descriptors. Then the client may not see the connection end when the server closes it, the program holds a socket it should never have, and the count of open files grows. TASK-1.113 reproduced the same race between tests: one write in four succeeded on a socket whose peer had been dropped while other threads forked. The launcher hands spawning to portable-pty, and nobody has read whether it closes inherited descriptors in the child. doc-8; doc-13 for what distribution needs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The notes say, from reading portable-pty's spawn at the pinned version and from a test, whether a pane's program can inherit a descriptor the server opened: an accepted connection, the listener, the lock file, another pane's pseudo-terminal
- [ ] #2 If it can: a test starts programs while connections are accepted and shows a program holding a connection's descriptor, with a recorded failing control, and the fix makes it pass; the fix closes or marks descriptors so that none but the program's own terminal reaches it
- [ ] #3 If it cannot: the notes show why with the lines read, and a test that lists a started program's open descriptors proves it holds only its terminal
<!-- AC:END -->

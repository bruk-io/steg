---
id: TASK-1.115
title: >-
  Server: scrollback in pages by row identity, and the restore proof after a
  client is killed
status: To Do
assignee: []
created_date: '2026-11-16T08:39:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.47
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-5
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.47 by its plan (2026-11-16). doc-9 R15, doc-8 R2, decision-2 sections 3 and 5. Covers TASK-1.47's second, fifth and sixth acceptance criteria. The kernel's side exists (Terminal::page and oldest_row, TASK-1.74). decision-2's text names the spike's path, which the deletion makes stale: note it on TASK-1.82.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Scrollback is fetched in pages by absolute row identity: a request names the rows it wants and the answer says which rows it holds and which no longer exist
- [ ] #2 A client killed in the middle of output, and a new one attaching afterwards and paging back, ends with the server's screen and scrollback byte for byte, as an automated test with a real pty, including output printed while no client was attached
- [ ] #3 The oracle of the wire-protocol spike is ported before spikes/task-1.2-wire-protocol/ is deleted, and the directory is deleted
- [ ] #4 Scrollback pages are offered by the steg library, and the tests go through it (R101)
<!-- AC:END -->

---
id: DRAFT-1
title: 'Server: a status that expires is reported to subscribers at its deadline'
status: Draft
assignee: []
created_date: '2026-11-16T12:11:00Z'
labels:
  - size-2
milestone: m-3
dependencies:
  - TASK-1.46
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.46 by the orchestrator on 2026-11-16 under the engineer's standing decision to split by default: that task's plan found that a finished status expiring ten minutes after it was set (doc-11 R82, R83; R52 as corrected) changes what a client should show with no event to tell it. crates/sdk/src/status.rs (TASK-1.41) deliberately carries no timing on the wire, so the server reports the expiry itself. Tree::next_status_deadline exists for this (crates/kernel/src/pane.rs). TASK-1.46's plan, section 6, holds the worked design: a thread beside the drain thread, woken by the deadline or by a change that moves it. Until this lands, a subscribed client sees a finished status until the next event for that pane.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 When a pane's finished status expires (R83) with no request and no change in the tree, subscribers receive pane.status_changed for it, and for its tab's shown status where that changes, at the deadline
- [ ] #2 One deadline is armed from the kernel's own next_status_deadline and re-armed when a change moves it; nothing wakes on a fixed interval, shown by a count of wake-ups and not by timing
- [ ] #3 The clock is injected: a test passes the ten-minute mark without waiting, and the boundary is tested just before and at the deadline
- [ ] #4 The thread that waits for the deadline is stopped and joined on every path out of the server, within a bound
<!-- AC:END -->

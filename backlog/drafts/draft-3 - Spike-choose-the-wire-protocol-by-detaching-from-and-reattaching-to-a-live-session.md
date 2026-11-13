---
id: DRAFT-3
title: >-
  Spike: choose the wire protocol by detaching from and reattaching to a live
  session
status: Draft
assignee: []
created_date: '2026-11-13T22:52:00Z'
labels:
  - size-3
dependencies: []
type: spike
parent_task_id: DRAFT-1
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Design decision 2 in the spec. A server holds one terminal; a client is killed and a new one reattaches with the screen and scrollback intact (R1, R2). Decides message framing, and whether terminal output reaches a client as raw bytes or as screen updates. The code is throwaway.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A board document records the framing and how terminal output reaches a client, with the reason
- [ ] #2 The document shows a client killed mid-output and a new client restoring the same screen and scrollback
- [ ] #3 The document says how the protocol carries a version, for R39
<!-- AC:END -->

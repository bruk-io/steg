---
id: TASK-1.62
title: 'App: status motion that can be dropped: the roll-out and the failure flakes'
status: To Do
assignee: []
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-15T02:15:00Z'
labels:
  - size-2
milestone: m-0
dependencies:
  - TASK-1.60
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-32
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-10 lists these as polish: the last things to build and the first to drop if they cost frames against R19. The app's look and behaviour is the design artifact (doc-10 says which page is the source for what). Until workers can read artifacts (asdlc intent workers-read-artifacts), the orchestrator hands over what the artifact says for this task.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A status bar rolls out when its pane comes into view, and flakes fall from a failed bar as the design shows
- [ ] #2 Neither happens with Reduce Motion on
- [ ] #3 The latency numbers of R19 are measured with both in place and still hold; if they do not, the motion is left out and the task says so
<!-- AC:END -->

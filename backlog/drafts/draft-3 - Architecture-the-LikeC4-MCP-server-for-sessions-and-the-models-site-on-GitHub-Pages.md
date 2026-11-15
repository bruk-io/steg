---
id: DRAFT-3
title: >-
  Architecture: the LikeC4 MCP server for sessions, and the model's site on
  GitHub Pages
status: Draft
assignee: []
created_date: '2026-11-15T02:48:00Z'
labels:
  - size-2
milestone: m-0
dependencies:
  - TASK-1.33
  - TASK-1.32
parent_task_id: TASK-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A follow-up to the LikeC4 model (TASK-1.33, doc-16 R108), asked for by Bruk on 2026-11-15. Two things: agents working in this repository can ask the model questions through LikeC4's MCP server instead of reading its source, and the built model is published as a site so it can be browsed without a checkout.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The repository configures LikeC4's MCP server for Claude Code sessions, pinned to the same LikeC4 version the check uses, and a session started in the repository can list the model's views and elements through it, shown by a recorded run
- [ ] #2 A workflow builds the model's static site and deploys it to GitHub Pages on every push to main that changes the model, with its actions and the LikeC4 version pinned
- [ ] #3 The site shows every view of the model, and its address is in the README and in doc-16
- [ ] #4 A model that does not validate fails the workflow and leaves the published site as it was
- [ ] #5 The plan states who can see the published site given the repository's visibility, and the engineer has confirmed that before anything is deployed
<!-- AC:END -->

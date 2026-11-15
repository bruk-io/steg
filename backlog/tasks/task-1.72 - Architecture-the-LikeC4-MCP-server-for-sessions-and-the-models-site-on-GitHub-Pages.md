---
id: TASK-1.72
title: 'Architecture: the LikeC4 MCP server for sessions'
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-15T02:48:00Z'
updated_date: '2026-11-15T03:21:00Z'
labels:
  - size-2
milestone: m-0
dependencies:
  - TASK-1.33
  - TASK-1.32
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-3
asdlc_branch: task-1.72-architecture-the-likec4-mcp-server-for
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A follow-up to the LikeC4 model (TASK-1.33, doc-16 R108), asked for by Bruk on 2026-11-15. Two things: agents working in this repository can ask the model questions through LikeC4's MCP server instead of reading its source, and the built model is published as a site so it can be browsed without a checkout.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The repository configures LikeC4's MCP server for Claude Code sessions, pinned to the same LikeC4 version the check uses, and a session started in the repository can list the model's views and elements through it, shown by a recorded run
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Decided by Bruk on 2026-11-15: it is fine for the architecture site to be public. The repository is private, so the GitHub Pages site will be readable by anyone with its address. The plan still states this, but it does not need confirming again.

Paused by Bruk on 2026-11-15: the GitHub Pages site is not part of this task for now. What was learned stays here for when it resumes: the repository is private and the bruk-io organisation is on GitHub's free plan, where Pages is not available for a private repository; a public site is acceptable to Bruk; the options were a separate public repository that receives only the built site, a paid plan, or making the repository public. The criteria removed were: a pinned workflow that builds and deploys the site on pushes to main that change the model; the site showing every view with its address in the README and doc-16; a model that does not validate failing the workflow and leaving the site as it was.
<!-- SECTION:NOTES:END -->

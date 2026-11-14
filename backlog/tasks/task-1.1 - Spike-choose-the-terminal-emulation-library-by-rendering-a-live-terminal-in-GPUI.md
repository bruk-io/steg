---
id: TASK-1.1
title: >-
  Spike: choose the terminal emulation library by rendering a live terminal in
  GPUI
status: In Progress
assignee:
  - '@bruk.w1'
created_date: '2026-11-13T22:52:00Z'
updated_date: '2026-11-14T01:01:00Z'
labels:
  - size-3
milestone: m-0
dependencies: []
parent_task_id: TASK-1
type: spike
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.1-spike-choose-the-terminal-emulation
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Answers intent question 3. Render one live shell in a GPUI window with libghostty-vt and with alacritty_terminal, and measure each against R19. Covers the risk in R14 and R19, and R59: no code from GPL sources, so nothing is copied from Zed's terminal crates. The code is throwaway; the answer is what this delivers.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A board document records the chosen library and why, with the measured 95th percentile typing latency and the result of printing a 100 MB file for each candidate
- [ ] #2 The document confirms or replaces the R19 numbers, which the spec marks as proposals
- [ ] #3 The document names the licence of every dependency the chosen approach needs
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Onboarding skipped by @bruk.o: bruk-io-steg has no repository plugin yet and no code to read layers or verify commands from. TASK-1.24 onboards it once the workspace exists. The repository's own rules are in .claude/rules/ and its commands are in the justfile.
<!-- SECTION:NOTES:END -->

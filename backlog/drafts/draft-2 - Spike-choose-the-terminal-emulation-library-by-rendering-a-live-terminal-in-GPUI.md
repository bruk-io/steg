---
id: DRAFT-2
title: >-
  Spike: choose the terminal emulation library by rendering a live terminal in
  GPUI
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
Answers intent question 3. Render one live shell in a GPUI window with libghostty-vt and with alacritty_terminal, and measure each against R19. Covers the risk in R14 and R19, and R59: no code from GPL sources, so nothing is copied from Zed's terminal crates. The code is throwaway; the answer is what this delivers.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A board document records the chosen library and why, with the measured 95th percentile typing latency and the result of printing a 100 MB file for each candidate
- [ ] #2 The document confirms or replaces the R19 numbers, which the spec marks as proposals
- [ ] #3 The document names the licence of every dependency the chosen approach needs
<!-- AC:END -->

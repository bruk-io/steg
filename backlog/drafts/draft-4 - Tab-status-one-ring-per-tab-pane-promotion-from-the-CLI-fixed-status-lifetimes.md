---
id: DRAFT-4
title: >-
  Tab status: one ring per tab, pane promotion from the CLI, fixed status
  lifetimes
status: Draft
assignee: []
created_date: '2026-11-14T23:24:00Z'
labels:
  - size-2
milestone: m-0
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Problem

A tab can hold several panes, but the bar shows one status ring per tab and none per session. Nothing in the spec says which pane's status a tab shows, how a program or an agent can choose it, or how long a status lasts.

Decided with Bruk in the design work on 2026-11-14 (design artifact, Foundations rules and the Tab card):

- A tab shows one ring for the most pressing of its panes: waiting, then failed, then bell, then finished, then running. With several panes running, it shows the one least far along, and unknown if any is unknown.
- A session shows no status of its own. Panes waiting for a person are counted once in the bar and listed first in the palette.
- A pane's status can be promoted to its tab from the CLI, so an agent can say which pane the tab's ring follows.
- A promoted pane decides what the ring follows while things are running or finished. A pane in the same tab that is waiting for a person, or has failed, still takes the ring.
- A pane may promote itself through the status protocol. Promoting a different pane is allowed from the CLI only.
- Each status type has a fixed lifetime, after which it ends without anyone acting.

## Proposal to confirm or change

Lifetimes, per status type (values are a first proposal, not decided):

- Running: lasts while the program keeps reporting; ends when it reports an end state or exits.
- Waiting: no limit; ends when the person answers or the program withdraws it.
- Failed: until the pane is made active, or a fixed time.
- Finished: until the pane is made active, or a fixed time.
- Bell: until the pane is made active, or a short fixed time.
- A promotion ends when the promoted pane's status ends; the tab then falls back to the most pressing rule.

## Open questions

- [ ] What is the fixed lifetime for failed, finished and bell, and does being seen end them sooner?
- [ ] Does running need a lifetime too, for a program that stops reporting without exiting?
- [ ] What is the CLI form for promotion, and does it take a pane target or act on the calling pane by default?
- [ ] Does promotion need its own message in the status protocol (OSC 7501), or a field on the existing status report?
- [ ] Is this M1, or does promotion belong with M2 (Extendable) while the one-ring rule and lifetimes stay in M1?
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 doc-1 states the one-ring-per-tab rule, that sessions carry no status, and the lifetime of each status type
- [ ] #2 doc-1 or the status protocol section says how a pane's status is promoted to its tab, who may do it, and when a promotion ends
- [ ] #3 doc-2 has the CLI command for promotion
- [ ] #4 A waiting or failed pane in the same tab still takes the ring over a promoted pane
<!-- AC:END -->

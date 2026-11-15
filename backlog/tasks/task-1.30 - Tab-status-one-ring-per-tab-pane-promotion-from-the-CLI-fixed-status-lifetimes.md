---
id: TASK-1.30
title: >-
  Tab status: one ring per tab, pane promotion from the CLI, fixed status
  lifetimes
status: To Do
assignee: []
created_date: '2026-11-14T23:24:00Z'
updated_date: '2026-11-15T00:23:00Z'
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
- [ ] #5 A status posted from the CLI joins the tab's set: it replaces only the same pane's previous status, and the tab shows the most pressing, then the next when one expires
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Open question added 2026-11-14 (Bruk): when the CLI posts a status, does it replace what is there or join a queue? Orchestrator's proposal, not decided: one status per pane, where a new report from that pane replaces its last; the tab then holds its panes' statuses as a set and shows the most pressing, falling to the next when one reaches the end of its lifetime. Ordered by how pressing, then newest, never first-in-first-out, so a waiting status is never stuck behind a finished one.

Decided 2026-11-14 (Bruk), replacing the open question above: status is a set, not a queue and not a single slot. (1) Within one pane a new report replaces that pane's last status; a pane has one status at a time. (2) A tab holds the current status of each of its panes and shows the most pressing. (3) When the status shown reaches the end of its lifetime, the next most pressing shows. (4) Order is how pressing (waiting, failed, bell, finished, running), then newest; never first-in-first-out. (5) The CLI adds to this set; it does not overwrite another pane's status. Lining up several messages to be shown one after another is notifications, not status, and is out of scope here.

Decided 2026-11-15 (Bruk): program status is split across releases. M1 shows only what steg can know without a program's help: running, and finished or failed from the exit code, using the ring and bar of doc-7. The Program Status Protocol, with progress, waiting for a person, the bell as a status, and promotion, stays in M2. doc-1's R49, R50, R52 and the release table are to be reworded for this.
<!-- SECTION:NOTES:END -->

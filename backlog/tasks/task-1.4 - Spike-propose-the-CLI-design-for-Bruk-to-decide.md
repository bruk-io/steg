---
id: TASK-1.4
title: 'Spike: propose the CLI design for Bruk to decide'
status: To Do
assignee: []
created_date: '2026-11-13T22:52:00Z'
updated_date: '2026-11-14T01:18:00Z'
labels:
  - size-2
milestone: m-0
dependencies: []
documentation:
  - doc-2
parent_task_id: TASK-1
type: spike
project: bruk-io-steg
asdlc_accepted_from: DRAFT-5
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Design decision 1 in the spec, which the spec deliberately leaves open. Proposes command names, flags, the environment variables that carry a terminal's context (R33), and exit statuses (R34), for the capabilities in R32 to R35 and the show-a-file command in R63.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A board document lists every proposed command with its flags, the context variables, and the exit statuses
- [ ] #2 The document marks each point where Rex's CLI does it differently, so the difference is a choice
- [ ] #3 Bruk has recorded his decision on the document
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From Bruk, 2026-11-13, before this was claimed: 'For the CLI we should keep the grammar unixy.' The proposal must take that as a constraint and say what it means in each choice: command shape (short verbs such as ls, split, send, capture, against generated noun-verb such as pane split), line-oriented output that pipes cleanly, silence on success, errors on stderr, standard short and long flags with -- ending options, reading standard input where a command takes content, and one way of writing a target. Recorded by @bruk.o; the reading of 'unixy' is the orchestrator's and is for Bruk to confirm in the proposal.

Bruk gave the CLI's design direction himself on 2026-11-13: centre it on speed, predictability and composability; noun-verb grammar; a consistent set of verbs; universal flags; positionals for the primary target and flags for everything else; boolean flags as switches with --no- pairs. The orchestrator wrote that up as the CLI specification (doc-2), with the team's cli-design skill as its policy. What is left for this task: settle the nine points in its section 9 with Bruk, check every command in its section 8 against the API description TASK-1.3 delivers, replace the proposed speed numbers with measured ones, and record Bruk's decision on the document. Recorded by @bruk.o.
<!-- SECTION:NOTES:END -->
